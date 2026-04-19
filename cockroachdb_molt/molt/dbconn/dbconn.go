package dbconn

import (
	"context"
	"net/url"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/lexbase"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/types"
	"github.com/cockroachdb/errors"
	"github.com/jackc/pgx/v5/pgtype"
	"github.com/lib/pq/oid"
)

type ID string

type OrderedConns [2]Conn

type Conn interface {
	ID() ID
	// Close closes the connection.
	Close(ctx context.Context) error
	// Clone creates a new Conn with the same underlying connections arguments.
	Clone(ctx context.Context) (Conn, error)
	// TypeMap returns a pgx typemap.
	TypeMap() *pgtype.Map

	IsCockroach() bool
	ConnStr() string
	Dialect() string
	Database() tree.Name
}

func Connect(ctx context.Context, preferredID ID, connStr string) (Conn, error) {
	id := preferredID
	if len(connStr) == 0 {
		return nil, errors.Newf("empty connection string")
	}

	before := strings.SplitN(connStr, "://", 2)

	switch {
	case strings.Contains(before[0], "postgres"):
		u, err := url.Parse(connStr)
		if err != nil {
			return nil, errors.Wrapf(err, "unable to parse url: %s", connStr)
		}

		if id == "" {
			id = ID(u.Hostname() + ":" + u.Port())
		}
		return ConnectPG(ctx, id, connStr)
	}
	return nil, errors.Newf("only postgres and postgresql schemes are supported, got %s from %s", before[0], connStr)
}

// TestOnlyCleanDatabase returns a connection to a clean database.
// This is recommended for test use only
func TestOnlyCleanDatabase(ctx context.Context, id ID, url string, dbName string) (Conn, error) {
	c, err := Connect(ctx, id, url)
	if err != nil {
		return nil, err
	}
	defer func() { _ = c.Close(ctx) }()

	switch c := c.(type) {
	case *PGConn:
		if _, err := c.Exec(ctx, "DROP DATABASE IF EXISTS "+lexbase.EscapeSQLIdent(dbName)); err != nil {
			return nil, err
		}
		if _, err := c.Exec(ctx, "CREATE DATABASE "+lexbase.EscapeSQLIdent(dbName)); err != nil {
			return nil, err
		}
		var version string
		if err := c.QueryRow(ctx, "SELECT version()").Scan(&version); err != nil {
			return nil, err
		}
		cfgCopy := c.Config().Copy()
		cfgCopy.Database = dbName
		return ConnectPGConfig(ctx, c.id, cfgCopy, true /*testOnly*/)
	}
	return nil, errors.AssertionFailedf("clean database only supported for PG connections, got %T", c)
}

func GetDataType(ctx context.Context, inConn Conn, oid oid.Oid) (*pgtype.Type, error) {
	if typ, ok := inConn.TypeMap().TypeForOID(uint32(oid)); ok {
		return typ, nil
	}
	if oid == types.AnyEnum.Oid() {
		return &pgtype.Type{
			Codec: &pgtype.EnumCodec{},
			Name:  "enum",
			OID:   uint32(oid),
		}, nil
	}
	conn, ok := inConn.(*PGConn)
	if !ok {
		return nil, errors.AssertionFailedf("only postgres types expected here, got %T, OID %d", conn, oid)
	}
	var typName string
	if err := conn.QueryRow(ctx, "SELECT $1::oid::regtype", oid).Scan(&typName); err != nil {
		return nil, errors.Wrapf(err, "error getting data type info for oid %d", oid)
	}
	typ, err := conn.LoadType(ctx, typName)
	if err != nil {
		return nil, errors.Wrapf(err, "error loading type %s", typName)
	}
	conn.TypeMap().RegisterType(typ)
	return typ, nil
}
