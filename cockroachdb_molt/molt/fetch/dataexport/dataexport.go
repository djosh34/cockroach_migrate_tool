package dataexport

import (
	"context"
	"fmt"
	"io"
	"os/exec"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/comparectx"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/moltcsv"
	"github.com/cockroachdb/molt/rowiterator"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/verify/rowverify"
	"github.com/rs/zerolog"
	"golang.org/x/sync/errgroup"
)

// DefaultReplicatorFlags are flags that are dialect
// agnostic ones that are passed into the replicator command.
var DefaultReplicatorFlags = map[string]string{"-v": ""}

type Source interface {
	CDCCursor() string
	Conn(ctx context.Context) (SourceConn, error)
	Close(ctx context.Context) error

	// HistoryRetention means to extend the expiration of a protected timestamp (PTS). A
	// SELECT ... AS OF SYSTEM TIME command with a PTS will not be invalidated by garbage collection.
	// A PTS is created when creating the
	// cockroachdb datasource (i.e. NewCRDBSource()) via the crdb_internal.protect_mvcc_history(<timestamp>, <lifetime>,
	// <description>).
	// At time t, if extend the history retention job, the job's expiration timestamp will be updated to {t + <lifetime>}.
	HistoryRetentionJobManagement(ctx context.Context, logger zerolog.Logger, exportFinished chan struct{}, renewInterval time.Duration, extensionCnt *int64, testOnly bool, ts *testutils.FetchTestingKnobs) (jobManagementFinished *errgroup.Group)
	ReplicatorCommand(bin string, target dbconn.Conn, db tree.Name, sc tree.Name, replicatorArgs string) (*exec.Cmd, error)
}

type SourceConn interface {
	Export(ctx context.Context, writer io.Writer, table dbtable.VerifiedTable, shard rowverify.TableShard) error
	Close(ctx context.Context) error
}

type Settings struct {
	RowBatchSize int

	PG PGReplicationSlotSettings

	CRDBPTSExtensionFreq     time.Duration
	CRDBPTSExtensionLifetime time.Duration
}

func InferExportSource(
	ctx context.Context, settings Settings, conn dbconn.Conn, logger zerolog.Logger, testOnly bool,
) (Source, error) {
	switch conn := conn.(type) {
	case *dbconn.PGConn:
		if conn.IsCockroach() {
			return NewCRDBSource(ctx, settings, conn, logger, testOnly)
		}
		return NewPGSource(ctx, settings, conn)
	case *dbconn.MySQLConn:
		return NewMySQLSource(ctx, settings, conn)
	}
	return nil, errors.AssertionFailedf("unknown conn type: %T", conn)
}

func scanWithRowIterator(
	ctx context.Context,
	settings Settings,
	c dbconn.Conn,
	writer io.Writer,
	table rowiterator.ScanTable,
) error {
	cw := moltcsv.NewWriter(writer)
	it, err := rowiterator.NewScanIterator(
		ctx,
		c,
		table,
		settings.RowBatchSize,
		nil,
	)
	if err != nil {
		return err
	}
	stringsToWrite := make([]string, 0, len(table.ColumnsWithAttr))
	quoteColumnIdxList := make([]string, 0)

	emptyStr := tree.DString("")
	for it.HasNext(ctx) {
		stringsToWrite = stringsToWrite[:0]
		datums := it.Next(ctx)
		var fmtFlags tree.FmtFlags

		// quoteColumnIdxList is to store the idx of entries in a row that should be quoted when being written to a csv.
		quoteColumnIdxList = quoteColumnIdxList[:0]

		for i, d := range datums {
			// FmtPgwireText is needed so that null columns get written as "" instead of string NULL
			// which happens inside f.FormatNode for type dNull datums.
			switch t := d.(type) {
			case *tree.DFloat:
				// With tree.FmtParsableNumerics, negative value will be bracketed, making it unable to be imported from
				// csv.
				fmtFlags = tree.FmtExport | tree.FmtPgwireText
			case *tree.DString:
				fmtFlags = tree.FmtExport | tree.FmtParsableNumerics | tree.FmtPgwireText
				compEmptyStrRes, err := t.CompareError(comparectx.CompareContext, &emptyStr)
				if err != nil {
					return errors.Wrapf(err, "error checking if the datum %s is an empty string", t)
				}
				// If this is an empty string, further check if this is a null.
				if compEmptyStrRes == 0 {
					compResForNull, err := t.CompareError(comparectx.CompareContext, tree.DNull)
					if err != nil {
						return errors.Wrapf(err, "error checking if the datum %s if of type tree.DNull", t)
					}
					// If this is a true empty string and not a null entry, write to the csv with double quotes.
					if compResForNull != 0 {
						quoteColumnIdxList = append(quoteColumnIdxList, strconv.Itoa(i))
					}
				}
			default:
				fmtFlags = tree.FmtExport | tree.FmtParsableNumerics | tree.FmtPgwireText
			}
			f := tree.NewFmtCtx(fmtFlags)
			f.FormatNode(d)
			stringsToWrite = append(stringsToWrite, f.CloseAndGetString())
		}
		if len(quoteColumnIdxList) > 0 {
			// If there are entries that should be written as double quotes, append the list of indexes of these entries
			// to the strings to write. i.e. for this row, there will be len(columns) + 1 entries in the csv pipe.
			// Note that in this stage, values are just pushed to the csv pipe, but not written to the csv file.
			// The writing to the csv file happen in `csvPipe.Pipe()`.
			quoteColumnIdxStr := fmt.Sprintf("%s%s", moltcsv.QuoteColumnIdxesPrefix, strings.Join(quoteColumnIdxList, moltcsv.QuoteColumnIdxesSeparator))
			stringsToWrite = append(stringsToWrite, quoteColumnIdxStr)
		}
		if err := cw.Write(stringsToWrite, nil); err != nil {
			return err
		}
	}
	if err := it.Error(); err != nil {
		return err
	}
	cw.Flush()
	return nil
}

func getFlagList(defaultFlags map[string]string, replicatorArgs string) ([]string, error) {
	overrideFlagList := parseFlagStrings(replicatorArgs)
	overrideFlagMapping, err := buildFlagMapFromSlice(overrideFlagList)
	if err != nil {
		return nil, err
	}

	return handleFlagOverrides(defaultFlags, overrideFlagMapping), nil
}

func parseFlagStrings(input string) []string {
	// This regex handles each argument but also cases
	// where there are spaces between args
	r := regexp.MustCompile(`(?:\"[^\"]+\"|\'[^\']+\'|\S+)`)
	m := r.FindAllString(input, -1)
	return m
}

func buildFlagMapFromSlice(input []string) (map[string]string, error) {
	output := map[string]string{}

	if len(input) == 0 {
		return output, nil
	} else if len(input) == 1 {
		if hasFlagPrefix(input[0]) {
			output[input[0]] = ""
		} else {
			return output, errors.Newf("invalid flag '%s'", input[0])
		}
		return output, nil
	}

	left := 0
	for left <= len(input)-1 {
		right := left + 1
		if hasFlagPrefix(input[left]) && !hasFlagPrefix(input[right]) {
			output[input[left]] = input[right]
			left += 2
		} else if hasFlagPrefix(input[left]) && hasFlagPrefix(input[right]) && right == len(input)-1 {
			output[input[left]] = ""
			output[input[right]] = ""
			break
		} else if hasFlagPrefix(input[left]) && hasFlagPrefix(input[right]) {
			output[input[left]] = ""
			left++
		} else {
			return map[string]string{}, errors.Newf("invalid flag '%s'", input[left])
		}
	}

	return output, nil
}

func hasFlagPrefix(input string) bool {
	return strings.HasPrefix(input, "--") || strings.HasPrefix(input, "-")
}

func handleFlagOverrides(defaultFlags, overrideFlags map[string]string) []string {
	finalFlags := []string{}

	for key, value := range overrideFlags {
		defaultFlags[key] = value
	}

	// Sort the keys.
	keys := make([]string, 0, len(defaultFlags))
	for k := range defaultFlags {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	// Iterate through the keys and add values in order.
	for _, k := range keys {
		val := defaultFlags[k]
		if val == "" {
			finalFlags = append(finalFlags, k)
		} else {
			finalFlags = append(finalFlags, k, val)
		}
	}

	return finalFlags
}
