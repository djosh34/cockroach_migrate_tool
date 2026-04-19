package replicator

import (
	"context"
	"fmt"
	"os"
	"path/filepath"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/verify/tableverify"
	"github.com/rs/zerolog"
)

func FindReplicatorBinary(start string) (string, error) {
	// TODO: Change once cdc-sink binaries are renamed to
	// replicator. Or we modify our release script to change the
	// name we bundle in our ZIP.
	p := filepath.Join(start, "cdc-sink")
	if _, err := os.Stat(p); err == nil {
		return p, nil
	}
	parent := filepath.Dir(start)
	if parent != start {
		return FindReplicatorBinary(parent)
	}
	// TODO: change this error message once cdc-sink binaries are renamed too.
	return "", errors.Newf("cdc-sink binary not found")
}

func SetupReplicator(
	ctx context.Context,
	conns dbconn.OrderedConns,
	tables []tableverify.Result,
	targetSchema tree.Name,
	cdcCursor string,
	logger zerolog.Logger,
) error {
	// If PG, we need to create a publication.
	// For CRDB, we need to create a changefeed
	if _, ok := conns[0].(*dbconn.PGConn); ok {
		cloneConn, err := conns[0].Clone(ctx)
		if err != nil {
			return err
		}

		cloneTargetConn, err := conns[1].Clone(ctx)

		if err != nil {
			return err
		}
		defer func() { _ = cloneConn.Close(ctx) }()
		defer func() { _ = cloneTargetConn.Close(ctx) }()
		conn := cloneConn.(*dbconn.PGConn).Conn
		connTarget := cloneTargetConn.(*dbconn.PGConn).Conn
		if conns[0].(*dbconn.PGConn).IsCockroach() {
			logger.Info().Msg("setting cluster setting kv.rangefeed.enabled to true")
			// We must have this cluster setting on to enable creating changefeeds
			if _, err := conn.Exec(ctx, "SET CLUSTER SETTING kv.rangefeed.enabled = true"); err != nil {
				return err
			}
			logger.Info().Msg("creating staging database")

			if _, err := connTarget.Exec(ctx, "CREATE DATABASE IF NOT EXISTS _cdc_sink"); err != nil {
				return err
			}

			logger.Info().Msg("seting zone ttl zone config")
			if _, err := connTarget.Exec(ctx, "ALTER DATABASE _cdc_sink CONFIGURE ZONE USING gc.ttlseconds=300"); err != nil {
				return err
			}
		} else { // If its not a CRDB conn then its a PG conn
			if _, err := conn.Exec(ctx, "DROP PUBLICATION IF EXISTS molt_fetch"); err != nil {
				return err
			}

			tblQ := CreateTableList(tables)
			logger.Info().Msg("creating publication")
			if _, err := conn.Exec(ctx, fmt.Sprintf("CREATE PUBLICATION molt_fetch FOR TABLE %s", tblQ)); err != nil {
				return err
			}
		}
	}
	return nil
}

func CreateTableList(tables []tableverify.Result) string {
	tblQ := ""
	for _, tbl := range tables {
		if tbl.RowVerifiable {
			if len(tblQ) > 0 {
				tblQ += ","
			}
			tblQ += fmt.Sprintf("%s.%s", tbl.Schema, tbl.Table)
		}
	}
	return tblQ
}
