package tableverify

import (
	"context"
	"database/sql"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/lib/pq/oid"
	"github.com/rs/zerolog"
)

type Column struct {
	Name      tree.Name
	OID       oid.Oid
	NotNull   bool
	Collation sql.NullString
}

func GetColumns(
	ctx context.Context, conn dbconn.Conn, table dbtable.DBTable, logger zerolog.Logger,
) ([]Column, error) {
	var ret []Column

	switch conn := conn.(type) {
	case *dbconn.PGConn:
		var defaultCollation string
		if err := conn.QueryRow(
			ctx,
			`SELECT pg_database.datcollate AS current_collation
FROM pg_catalog.pg_database
WHERE pg_database.datname = pg_catalog.current_database()`,
		).Scan(&defaultCollation); err != nil {
			return ret, nil
		}
		rows, err := conn.Query(
			ctx,
			`SELECT
attname, atttypid, attnotnull, collname
FROM pg_attribute
LEFT OUTER JOIN pg_collation ON (pg_collation.oid = pg_attribute.attcollation)
WHERE attrelid = $1 AND attnum > 0 AND attisdropped = false
ORDER BY attnum`,
			table.OID,
		)
		if err != nil {
			return ret, err
		}

		for rows.Next() {
			var cm Column
			if err := rows.Scan(&cm.Name, &cm.OID, &cm.NotNull, &cm.Collation); err != nil {
				return ret, errors.Wrap(err, "error decoding column metadata")
			}
			if !cm.Collation.Valid || cm.Collation.String == "default" {
				cm.Collation.String = defaultCollation
				cm.Collation.Valid = true
			}
			ret = append(ret, cm)
		}
		if rows.Err() != nil {
			return ret, errors.Wrap(err, "error collecting column metadata")
		}
		rows.Close()
	default:
		return nil, errors.Newf("only PG connections are supported, got %T", conn)
	}
	return ret, nil
}

func getColumnsForTables(
	ctx context.Context, conns dbconn.OrderedConns, logger zerolog.Logger, tbls [2]dbtable.DBTable,
) ([2][]Column, error) {
	var ret [2][]Column
	for i, conn := range conns {
		var err error
		ret[i], err = GetColumns(ctx, conn, tbls[i], logger)
		if err != nil {
			return ret, err
		}
	}
	return ret, nil
}

func getPrimaryKeysForTables(
	ctx context.Context, conns dbconn.OrderedConns, tbls [2]dbtable.DBTable,
) ([2][]tree.Name, error) {
	var ret [2][]tree.Name
	for i, conn := range conns {
		var err error
		ret[i], err = getPrimaryKey(ctx, conn, tbls[i])
		if err != nil {
			return ret, err
		}
	}
	return ret, nil
}

func getPrimaryKey(
	ctx context.Context, conn dbconn.Conn, table dbtable.DBTable,
) ([]tree.Name, error) {
	var ret []tree.Name

	switch conn := conn.(type) {
	case *dbconn.PGConn:
		rows, err := conn.Query(
			ctx,
			`
SELECT
    a.attname AS column_name
FROM
    pg_index i
JOIN
    pg_attribute a ON a.attrelid = i.indrelid
AND
    a.attnum = ANY(i.indkey)
WHERE
    i.indrelid = $1
AND
    i.indisprimary
ORDER BY
    array_position(i.indkey, a.attnum);
`,
			table.OID,
		)
		if err != nil {
			return ret, err
		}

		for rows.Next() {
			var c tree.Name
			if err := rows.Scan(&c); err != nil {
				return ret, errors.Wrap(err, "error decoding column name")
			}
			ret = append(ret, c)
		}
		if rows.Err() != nil {
			return ret, errors.Wrap(err, "error collecting primary key")
		}
		rows.Close()
	default:
		return nil, errors.AssertionFailedf("only PG connections are supported, got %T", conn)
	}
	return ret, nil
}

func mapColumns(cols []Column) map[tree.Name]Column {
	ret := make(map[tree.Name]Column, len(cols))
	for _, col := range cols {
		ret[col.Name] = col
	}
	return ret
}
