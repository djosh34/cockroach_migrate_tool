package dataexport

import (
	"context"
	"fmt"
	"io"
	"os/exec"
	"strconv"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/rowiterator"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/rowverify"
	"github.com/rs/zerolog"
	"golang.org/x/sync/errgroup"
)

type crdbSource struct {
	aost time.Time

	hrJobID  string
	settings Settings
	conn     dbconn.Conn
}

func NewCRDBSource(
	ctx context.Context, settings Settings, conn *dbconn.PGConn, logger zerolog.Logger, testOnly bool,
) (*crdbSource, error) {

	isAfter241, err := conn.CheckIfAfterVersion(ctx, "24.1")
	if err != nil {
		return nil, err
	}

	ts := time.Now().UTC().Truncate(time.Second)

	res := &crdbSource{
		conn:     conn,
		settings: settings,
		aost:     ts,
	}

	if isAfter241 {
		if settings.CRDBPTSExtensionLifetime == 0 {
			return nil, errors.AssertionFailedf("crdb-pts-duration must be greater than 0")
		}
		newConn, err := conn.Clone(ctx)
		if err != nil {
			return nil, errors.Wrapf(err, "failed to clone a connection to enforce history retention")
		}
		defer func() { newConn.Close(ctx) }()
		var jobID int64
		pgConn := newConn.(*dbconn.PGConn)
		unixTime := ts.Unix()
		query := fmt.Sprintf("SELECT crdb_internal.protect_mvcc_history('%d', '%s'::interval, 'molt fetch data export')", unixTime, settings.CRDBPTSExtensionLifetime.String())
		logger.Debug().Msgf("running history retention job with: %s", query)
		err = pgConn.QueryRow(ctx, query).Scan(&jobID)
		if err != nil {
			return nil, errors.WithHintf(
				errors.Wrapf(err, "failed to protect timestamp for %s", ts),
				"does your crdb source db support crdb_internal.protect_mvcc_history() built-in function?",
			)
		}
		logger.Info().Msgf("timestamp is protected for %s: timestamp: %d", settings.CRDBPTSExtensionLifetime.String(), utils.MaybeFormatTimestamp(testOnly, unixTime))
		res.hrJobID = strconv.FormatInt(jobID, 10)
	} else {
		logger.Warn().Msgf("crdb version might not support history retention for specific timestamp. AS OF SYSTEM TIME with timestamp %d might fail for long export. Try setting a larger value for gc.ttlseconds, see also https://www.cockroachlabs.com/docs/stable/as-of-system-time", utils.MaybeFormatTimestamp(testOnly, ts.Unix()))
	}
	return res, nil
}

func (c *crdbSource) resetJobId() {
	c.hrJobID = ""
}

func (c *crdbSource) CDCCursor() string {
	return c.aost.Format(time.RFC3339Nano)
}

func (c *crdbSource) Conn(ctx context.Context) (SourceConn, error) {
	conn, err := c.conn.Clone(ctx)
	if err != nil {
		return nil, err
	}
	return &crdbSourceConn{conn: conn, src: c}, nil
}

func (c *crdbSource) Close(ctx context.Context) error {
	return nil
}

type crdbSourceConn struct {
	conn dbconn.Conn
	src  *crdbSource
}

func (c *crdbSourceConn) Export(
	ctx context.Context, writer io.Writer, table dbtable.VerifiedTable, shard rowverify.TableShard,
) error {
	return scanWithRowIterator(ctx, c.src.settings, c.conn, writer, rowiterator.ScanTable{
		Table: rowiterator.Table{
			Name:              table.Name,
			ColumnsWithAttr:   table.Columns,
			ColumnOIDs:        table.ColumnOIDs[0],
			PrimaryKeyColumns: table.PrimaryKeyColumns,
		},
		AOST:        &c.src.aost,
		StartPKVals: shard.StartPKVals,
		EndPKVals:   shard.EndPKVals,
	})
}

func (c *crdbSourceConn) Close(ctx context.Context) error {
	return c.conn.Close(ctx)
}

func (c *crdbSource) ReplicatorCommand(
	bin string, target dbconn.Conn, db tree.Name, sc tree.Name, replicatorArgs string,
) (*exec.Cmd, error) {
	additionalFlagList, err := getFlagList(DefaultReplicatorFlags, replicatorArgs)
	if err != nil {
		return nil, err
	}

	cmdArgs := []string{
		bin,
		"start",
		"--bindAddr", "0.0.0.0:30004",
		"--tlsSelfSigned",
		"--disableAuthentication",
		"--targetConn", target.ConnStr(),
	}
	cmdArgs = append(cmdArgs, additionalFlagList...)
	return exec.Command(cmdArgs[0], cmdArgs[1:]...), nil
}

func (c *crdbSource) cancelRetentionJob(
	ctx context.Context, logger zerolog.Logger, testOnly bool,
) error {
	if c.hrJobID == "" {
		return nil
	}
	defer func() {
		c.resetJobId()
		logger.Debug().Msgf("done resetting history retention job id")
	}()

	pgConn := c.conn.(*dbconn.PGConn)
	cn, err := pgConn.Clone(ctx)
	if err != nil {
		return errors.Wrapf(err, "failed to clone a connection to cancel history retention job")
	}
	defer cn.Close(ctx)
	conn := cn.(*dbconn.PGConn)

	if _, err := conn.Exec(ctx, `CANCEL JOB $1`, c.hrJobID); err != nil {
		msg := "failed extending protection interval"
		logger.Warn().Msgf(msg)
		return errors.Wrapf(err, msg)
	}
	logger.Info().Msgf("cancelled history retention job")
	return nil
}

func (c *crdbSource) HistoryRetentionJobManagement(
	ctx context.Context,
	logger zerolog.Logger,
	exportFinished chan struct{},
	renewInterval time.Duration,
	extensionCnt *int64,
	testOnly bool,
	ts *testutils.FetchTestingKnobs,
) *errgroup.Group {
	if c.hrJobID == "" {
		return nil
	}

	if ts != nil && ts.HistoryRetention != nil && ts.HistoryRetention.JobID != nil {
		*ts.HistoryRetention.JobID = c.hrJobID
	}

	loggerWithHrJobID := logger.With().Str("hr_job", utils.MaybeFormatHistoryRetentionJobID(testOnly, c.hrJobID)).Logger()

	ticker := time.NewTicker(renewInterval)
	wg, ctx := errgroup.WithContext(ctx)
	wg.Go(func() (retErr error) {
		pgConn := c.conn.(*dbconn.PGConn)
		defer func() {
			retErr = errors.CombineErrors(retErr, c.cancelRetentionJob(ctx, loggerWithHrJobID, testOnly))
		}()
		for {
			breakLoop, err := func() (breakLoop bool, retErr error) {
				select {
				// If the export job has finished, we cancel the history retention job and exit the goroutine.
				case <-exportFinished:
					return true, c.cancelRetentionJob(ctx, loggerWithHrJobID, testOnly)
				// If timeout, we extend the history retention job and enter the next loop.
				case <-ticker.C:
					newConn, err := pgConn.Clone(ctx)
					if err != nil {
						msg := "failed cloning new connection to for history retention check"
						// This will be immediately logged while export is ongoing.
						loggerWithHrJobID.Err(err).Msgf(msg)
						return true, errors.Wrapf(err, msg)
					}
					defer func() {
						retErr = errors.CombineErrors(retErr, newConn.Close(ctx))
					}()
					newPgConn := newConn.(*dbconn.PGConn)
					if _, err := newPgConn.Exec(ctx, `SELECT crdb_internal.extend_mvcc_history_protection($1)`, c.hrJobID); err != nil {
						msg := "failed extending protection interval"
						loggerWithHrJobID.Warn().Msgf(msg)
						return true, errors.Wrapf(err, msg)
					}
					*extensionCnt++
					loggerWithHrJobID.Info().Msgf("has done %d history retention interval extention", *extensionCnt)
					return false, nil

				}
			}()
			if err != nil {
				return err
			}
			if breakLoop {
				break
			}
		}
		loggerWithHrJobID.Debug().Msgf("exiting history retention management go routine")
		return nil
	})
	return wg
}
