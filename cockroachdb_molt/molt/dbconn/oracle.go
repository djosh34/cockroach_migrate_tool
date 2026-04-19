package dbconn

import (
	"context"
	"database/sql"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/jackc/pgx/v5/pgtype"
	_ "github.com/sijms/go-ora/v2"
)

type OracleConn struct {
	id      ID
	connStr string
	typeMap *pgtype.Map
	*sql.DB
	database tree.Name
}

var _ Conn = (*OracleConn)(nil)

func ConnectOracle(ctx context.Context, id ID, connStr string) (*OracleConn, error) {
	db, err := sql.Open("oracle", connStr)
	if err != nil {
		return nil, err
	}
	return &OracleConn{
		id:      id,
		connStr: connStr,
		DB:      db,
		typeMap: pgtype.NewMap(),
	}, nil
}

func (c *OracleConn) Close(ctx context.Context) error {
	return c.DB.Close()
}

func (c *OracleConn) ID() ID {
	return c.id
}

func (c *OracleConn) IsCockroach() bool {
	return false
}

func (c *OracleConn) Clone(ctx context.Context) (Conn, error) {
	ret, err := ConnectOracle(ctx, c.id, c.connStr)
	if err != nil {
		return nil, err
	}
	ret.typeMap = c.typeMap
	return ret, nil
}

func (c *OracleConn) ConnStr() string {
	return c.connStr
}

func (c *OracleConn) Dialect() string {
	return "Oracle"
}

func (c *OracleConn) TypeMap() *pgtype.Map {
	return c.typeMap
}

func (c *OracleConn) Database() tree.Name {
	return c.database
}
