package dbconn

import (
	"context"
	"regexp"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/types"
	"github.com/cockroachdb/errors"
	"github.com/jackc/pgx/v5"
	"github.com/lib/pq/oid"
)

var defaultDBRegex = regexp.MustCompile(`defaultdb`)

type PGConn struct {
	id ID
	*pgx.Conn
	version     string
	connStr     string
	isCockroach bool
	database    tree.Name
	testOnly    bool
}

var _ Conn = (*PGConn)(nil)

func ConnectPG(ctx context.Context, id ID, connStr string) (*PGConn, error) {
	cfg, err := pgx.ParseConfig(connStr)
	if err != nil {
		return nil, err
	}
	return ConnectPGConfig(ctx, id, cfg, false /* testOnly */)
}

func ConnectPGConfig(
	ctx context.Context, id ID, cfg *pgx.ConnConfig, testOnly bool,
) (*PGConn, error) {
	conn, err := pgx.ConnectConfig(ctx, cfg)
	if err != nil {
		return nil, errors.Wrapf(err, "error connect")
	}
	var version string
	if err := conn.QueryRow(ctx, "SELECT version()").Scan(&version); err != nil {
		return nil, err
	}

	connStr := cfg.ConnString()

	if testOnly {
		// In the test only case the root database template has a database
		// name of defaultdb.
		// In the case that the database name is defaultdb, this is a no-op.
		// In the other cases, it ensures that the database name in the string is the
		// same as the config. The reason this is needed is that cfg.ConnString
		// in pgx keys off the original conn string, which will be defaultdb
		// in the test connection case.
		// Additionally, we don't expect the database name override when using
		// fetch via the binary and not in a test.
		connStr = defaultDBRegex.ReplaceAllString(cfg.ConnString(), cfg.Database)
	}

	return &PGConn{
		id:          id,
		Conn:        conn,
		version:     version,
		connStr:     connStr,
		isCockroach: strings.Contains(version, "CockroachDB"),
		database:    tree.Name(cfg.Database),
	}, nil
}

func (c *PGConn) ID() ID {
	return c.id
}

func (c *PGConn) IsCockroach() bool {
	return c.isCockroach
}

func (c *PGConn) Clone(ctx context.Context) (Conn, error) {
	return ConnectPGConfig(ctx, c.id, c.Conn.Config(), c.testOnly)
}

func (c *PGConn) ConnStr() string {
	return c.connStr
}

func init() {
	// Inject JSON as a OidToType.
	types.OidToType[oid.T_json] = types.Jsonb
	types.OidToType[oid.T__json] = types.MakeArray(types.Jsonb)
}

func (c *PGConn) Dialect() string {
	if c.IsCockroach() {
		return "CockroachDB"
	}
	return "PostgreSQL"
}

func (c *PGConn) Database() tree.Name {
	return c.database
}
