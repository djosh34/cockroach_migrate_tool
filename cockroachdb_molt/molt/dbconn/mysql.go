package dbconn

import (
	"context"
	"database/sql"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/molt/mysqlurl"
	"github.com/go-sql-driver/mysql"
	"github.com/jackc/pgx/v5/pgtype"
)

type MySQLConn struct {
	id     ID
	url    string
	tlsMap map[string]string
	*sql.DB
	typeMap  *pgtype.Map
	database tree.Name
}

func ConnectMySQL(ctx context.Context, id ID, connStr string) (*MySQLConn, error) {
	cfg, err := mysqlurl.Parse(connStr)
	if err != nil {
		return nil, err
	}

	tlsMap := handleTLSParams(cfg)
	u := cfg.FormatDSN()
	db, err := sql.Open("mysql", u)
	if err != nil {
		return nil, err
	}
	m := pgtype.NewMap()
	return &MySQLConn{id: id, url: u, DB: db, typeMap: m, database: tree.Name(cfg.DBName), tlsMap: tlsMap}, nil
}

func handleTLSParams(cfg *mysql.Config) map[string]string {
	tlsParams := []string{"sslmode", "sslrootcert", "sslcert", "sslkey"}
	tlsMap := make(map[string]string, 4)

	for _, paramName := range tlsParams {
		if v, ok := cfg.Params[paramName]; ok {
			tlsMap[paramName] = v
			delete(cfg.Params, paramName)
		}
	}

	return tlsMap
}

func (c *MySQLConn) ID() ID {
	return c.id
}

func (c *MySQLConn) Close(ctx context.Context) error {
	return c.DB.Close()
}

func (c *MySQLConn) Clone(ctx context.Context) (Conn, error) {
	ret, err := ConnectMySQL(ctx, c.id, c.url)
	if err != nil {
		return nil, err
	}
	ret.typeMap = c.typeMap
	return ret, nil
}

func (c *MySQLConn) TypeMap() *pgtype.Map {
	return c.typeMap
}

func (c *MySQLConn) IsCockroach() bool {
	return false
}

func (c *MySQLConn) ConnStr() string {
	return c.url
}

var _ Conn = (*MySQLConn)(nil)

func (c *MySQLConn) Dialect() string {
	return "MySQL"
}

func (c *MySQLConn) Database() tree.Name {
	return c.database
}

func (c *MySQLConn) TLSMap() map[string]string {
	return c.tlsMap
}
