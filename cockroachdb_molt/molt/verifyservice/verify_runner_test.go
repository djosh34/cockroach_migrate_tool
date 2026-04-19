package verifyservice

import (
	"context"
	"testing"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/jackc/pgx/v5/pgtype"
	"github.com/rs/zerolog"
	"github.com/stretchr/testify/require"
)

func TestVerifyRunnerUsesConfigConnectionStringsAndTreatsRequestFiltersAsData(t *testing.T) {
	t.Parallel()

	cfg := Config{
		Verify: VerifyConfig{
			Source: DatabaseConfig{
				URL: "postgres://source-user:source-pass@source-db:26257/source_db?application_name=verify",
				TLS: DatabaseTLSConfig{
					Mode:           DBTLSModeVerifyFull,
					CACertPath:     "/etc/source-ca.pem",
					ClientCertPath: "/etc/source-client.pem",
					ClientKeyPath:  "/etc/source-client.key",
				},
			},
			Destination: DatabaseConfig{
				URL: "postgres://target-user:target-pass@target-db:26257/target_db?application_name=verify",
				TLS: DatabaseTLSConfig{
					Mode:       DBTLSModeVerifyCA,
					CACertPath: "/etc/target-ca.pem",
				},
			},
		},
	}
	sourceConnStr, err := cfg.Verify.Source.ConnectionString()
	require.NoError(t, err)
	destinationConnStr, err := cfg.Verify.Destination.ConnectionString()
	require.NoError(t, err)

	request, err := (JobRequest{
		Filters: JobFilters{
			Include: NameFilters{
				Schema: "^public$",
				Table:  "accounts;$(touch /tmp/pwned)|orders",
			},
			Exclude: NameFilters{
				Schema: "audit|tmp;rm -rf /",
				Table:  "^tmp_",
			},
		},
	}).Compile()
	require.NoError(t, err)

	var gotConnectCalls []struct {
		id      dbconn.ID
		connStr string
	}
	var gotFilter utils.FilterConfig

	runner := VerifyRunner{
		config: cfg,
		logger: zerolog.Nop(),
		connect: func(_ context.Context, preferredID dbconn.ID, connStr string) (dbconn.Conn, error) {
			gotConnectCalls = append(gotConnectCalls, struct {
				id      dbconn.ID
				connStr string
			}{
				id:      preferredID,
				connStr: connStr,
			})
			return fakeConn{id: preferredID, connStr: connStr}, nil
		},
		runVerify: func(
			_ context.Context,
			_ dbconn.OrderedConns,
			_ zerolog.Logger,
			_ inconsistency.Reporter,
			filter utils.FilterConfig,
		) error {
			gotFilter = filter
			return nil
		},
	}

	err = runner.Run(context.Background(), request, noopReporter{})
	require.NoError(t, err)
	require.Equal(t, []struct {
		id      dbconn.ID
		connStr string
	}{
		{id: "source", connStr: sourceConnStr},
		{id: "target", connStr: destinationConnStr},
	}, gotConnectCalls)
	require.Equal(t, utils.FilterConfig{
		SchemaFilter:        "^public$",
		TableFilter:         "accounts;$(touch /tmp/pwned)|orders",
		ExcludeSchemaFilter: "audit|tmp;rm -rf /",
		ExcludeTableFilter:  "^tmp_",
	}, gotFilter)
}

type fakeConn struct {
	id      dbconn.ID
	connStr string
}

func (c fakeConn) ID() dbconn.ID {
	return c.id
}

func (fakeConn) Close(context.Context) error {
	return nil
}

func (c fakeConn) Clone(context.Context) (dbconn.Conn, error) {
	return c, nil
}

func (fakeConn) TypeMap() *pgtype.Map {
	return pgtype.NewMap()
}

func (fakeConn) IsCockroach() bool {
	return true
}

func (c fakeConn) ConnStr() string {
	return c.connStr
}

func (fakeConn) Dialect() string {
	return "postgres"
}

func (fakeConn) Database() tree.Name {
	return tree.Name("test")
}

type noopReporter struct{}

func (noopReporter) Report(inconsistency.ReportableObject) {}

func (noopReporter) Close() {}
