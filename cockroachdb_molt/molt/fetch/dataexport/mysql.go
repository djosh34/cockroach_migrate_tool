package dataexport

import (
	"context"
	"database/sql"
	"fmt"
	"io"
	"os/exec"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/mysqlurl"
	"github.com/cockroachdb/molt/rowiterator"
	"github.com/cockroachdb/molt/testutils"
	"github.com/cockroachdb/molt/verify/rowverify"
	"github.com/rs/zerolog"
	"golang.org/x/sync/errgroup"
)

const GTIDHelpInstructions = `please ensure that you have GTID-based replication enabled`

type mysqlSource struct {
	gtid     string
	settings Settings
	conn     dbconn.Conn
}

func NewMySQLSource(
	ctx context.Context, settings Settings, conn *dbconn.MySQLConn,
) (*mysqlSource, error) {
	var source string
	var start, end int
	if err := func() error {
		if err := conn.QueryRowContext(ctx, "select source_uuid, min(interval_start), max(interval_end) from mysql.gtid_executed group by source_uuid").Scan(
			&source, &start, &end,
		); err != nil {
			return errors.Wrapf(err, "failed to export snapshot: %s", GTIDHelpInstructions)
		}
		return nil
	}(); err != nil {
		return nil, err
	}
	return &mysqlSource{
		gtid:     fmt.Sprintf("%s:%d-%d", source, start, end),
		conn:     conn,
		settings: settings,
	}, nil
}

func (m *mysqlSource) CDCCursor() string {
	return m.gtid
}

func (m *mysqlSource) Close(ctx context.Context) error {
	return nil
}

func (m *mysqlSource) Conn(ctx context.Context) (SourceConn, error) {
	conn, err := m.conn.Clone(ctx)
	if err != nil {
		return nil, err
	}
	tx, err := conn.(*dbconn.MySQLConn).BeginTx(ctx, &sql.TxOptions{
		Isolation: sql.LevelRepeatableRead,
		ReadOnly:  true,
	})
	if err != nil {
		return nil, errors.CombineErrors(err, conn.Close(ctx))
	}
	return &mysqlConn{
		conn: conn,
		tx:   tx,
		src:  m,
	}, nil
}

type mysqlConn struct {
	conn dbconn.Conn
	tx   *sql.Tx
	src  *mysqlSource
}

func (m *mysqlConn) Export(
	ctx context.Context, writer io.Writer, table dbtable.VerifiedTable, shard rowverify.TableShard,
) error {
	return scanWithRowIterator(ctx, m.src.settings, m.conn, writer, rowiterator.ScanTable{
		Table: rowiterator.Table{
			Name:              table.Name,
			ColumnsWithAttr:   table.Columns,
			ColumnOIDs:        table.ColumnOIDs[0],
			PrimaryKeyColumns: table.PrimaryKeyColumns,
		},
		StartPKVals: shard.StartPKVals,
		EndPKVals:   shard.EndPKVals,
	})
}

func (m *mysqlConn) Close(ctx context.Context) error {
	return m.conn.Close(ctx)
}

func (c *mysqlSource) ReplicatorCommand(
	bin string, target dbconn.Conn, db tree.Name, sc tree.Name, replicatorArgs string,
) (*exec.Cmd, error) {
	mysqlConfig, err := mysqlurl.Parse(c.conn.ConnStr())
	if err != nil {
		return nil, err
	}

	mysqlConn, ok := c.conn.(*dbconn.MySQLConn)
	if !ok {
		return nil, errors.New("failed to cast to a MySQL connection")
	}

	// Add TLS parameters back to the configuration to pass to CDC sink.
	if mysqlConfig.Params == nil {
		// In the case that TLS map is also nil or an empty map
		// no extra parameters will be added to the connection string anyways.
		mysqlConfig.Params = mysqlConn.TLSMap()
	} else {
		for k, v := range mysqlConn.TLSMap() {
			mysqlConfig.Params[k] = v
		}
	}

	additionalFlagList, err := getFlagList(DefaultReplicatorFlags, replicatorArgs)
	if err != nil {
		return nil, err
	}

	sourceConn := mysqlurl.CfgToConnStr(mysqlConfig, true)
	if mysqlConfig.TLS == nil {
		sourceConn += "?sslmode=disable"
	}

	cmdArgs := []string{
		bin,
		"mylogical",
		"--sourceConn", sourceConn,
		"--targetConn", target.ConnStr(),
		"--defaultGTIDSet", c.gtid,
		"--targetSchema", fmt.Sprintf("%s.%s", db, sc),
		"--stagingSchema", fmt.Sprint(db),
		"-v",
	}
	cmdArgs = append(cmdArgs, additionalFlagList...)
	return exec.Command(cmdArgs[0], cmdArgs[1:]...), nil
}

// Unimplemented
func (m *mysqlSource) HistoryRetentionJobManagement(
	ctx context.Context,
	logger zerolog.Logger,
	exportFinished chan struct{},
	renewInterval time.Duration,
	extensionCnt *int64,
	testOnly bool,
	ts *testutils.FetchTestingKnobs,
) *errgroup.Group {
	return nil
}
