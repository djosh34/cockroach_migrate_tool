package verifyservice

import (
	"context"
	"errors"
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
				URL:            "postgres://source-user:source-pass@source-db:26257/source_db?application_name=verify&sslmode=verify-full",
				CACertPath:     "/etc/source-ca.pem",
				ClientCertPath: "/etc/source-client.pem",
				ClientKeyPath:  "/etc/source-client.key",
			},
			Destination: DatabaseConfig{
				URL:        "postgres://target-user:target-pass@target-db:26257/target_db?application_name=verify&sslmode=verify-ca",
				CACertPath: "/etc/target-ca.pem",
			},
		},
	}
	sourceConnStr, err := cfg.Verify.Source.ConnectionString()
	require.NoError(t, err)
	destinationConnStr, err := cfg.Verify.Destination.ConnectionString()
	require.NoError(t, err)

	request, err := (JobRequest{
		IncludeSchema: "^public$",
		IncludeTable:  "accounts;$(touch /tmp/pwned)|orders",
		ExcludeSchema: "audit|tmp;rm -rf /",
		ExcludeTable:  "^tmp_",
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

func TestVerifyRunnerClassifiesSourceConnectionFailures(t *testing.T) {
	t.Parallel()

	request, err := (JobRequest{}).Compile()
	require.NoError(t, err)

	runner := VerifyRunner{
		config: Config{
			Verify: VerifyConfig{
				Source: DatabaseConfig{
					URL: "postgres://verify_source:wrong-secret@source-db:26257/source_db?application_name=verify",
				},
				Destination: DatabaseConfig{
					URL: "postgres://verify_target:correct-secret@target-db:26257/target_db?application_name=verify",
				},
			},
		},
		logger: zerolog.Nop(),
		connect: func(_ context.Context, preferredID dbconn.ID, _ string) (dbconn.Conn, error) {
			if preferredID == "source" {
				return nil, errors.New("password authentication failed for user verify_source")
			}
			return fakeConn{id: preferredID}, nil
		},
		runVerify: func(
			_ context.Context,
			_ dbconn.OrderedConns,
			_ zerolog.Logger,
			_ inconsistency.Reporter,
			_ utils.FilterConfig,
		) error {
			t.Fatal("runVerify should not be called when source connection setup fails")
			return nil
		},
	}

	err = runner.Run(context.Background(), request, noopReporter{})
	require.Equal(
		t,
		&operatorError{
			category: "source_access",
			code:     "connection_failed",
			message:  "source connection failed: password authentication failed for user verify_source",
			details: []operatorErrorDetail{
				{Reason: "password authentication failed for user verify_source"},
			},
		},
		err,
	)
}

func TestVerifyRunnerClassifiesDestinationConnectionFailures(t *testing.T) {
	t.Parallel()

	request, err := (JobRequest{}).Compile()
	require.NoError(t, err)

	runner := VerifyRunner{
		config: Config{
			Verify: VerifyConfig{
				Source: DatabaseConfig{
					URL: "postgres://verify_source:correct-secret@source-db:26257/source_db?application_name=verify",
				},
				Destination: DatabaseConfig{
					URL: "postgres://verify_target:wrong-secret@target-db:26257/target_db?application_name=verify",
				},
			},
		},
		logger: zerolog.Nop(),
		connect: func(_ context.Context, preferredID dbconn.ID, _ string) (dbconn.Conn, error) {
			if preferredID == "target" {
				return nil, errors.New("password authentication failed for user verify_target")
			}
			return fakeConn{id: preferredID}, nil
		},
		runVerify: func(
			_ context.Context,
			_ dbconn.OrderedConns,
			_ zerolog.Logger,
			_ inconsistency.Reporter,
			_ utils.FilterConfig,
		) error {
			t.Fatal("runVerify should not be called when destination connection setup fails")
			return nil
		},
	}

	err = runner.Run(context.Background(), request, noopReporter{})
	require.Equal(
		t,
		&operatorError{
			category: "destination_access",
			code:     "connection_failed",
			message:  "destination connection failed: password authentication failed for user verify_target",
			details: []operatorErrorDetail{
				{Reason: "password authentication failed for user verify_target"},
			},
		},
		err,
	)
}

func TestVerifyRunnerClassifiesVerifyExecutionFailures(t *testing.T) {
	t.Parallel()

	request, err := (JobRequest{}).Compile()
	require.NoError(t, err)

	runner := VerifyRunner{
		config: Config{
			Verify: VerifyConfig{
				Source: DatabaseConfig{
					URL: "postgres://verify_source:correct-secret@source-db:26257/source_db?application_name=verify",
				},
				Destination: DatabaseConfig{
					URL: "postgres://verify_target:correct-secret@target-db:26257/target_db?application_name=verify",
				},
			},
		},
		logger: zerolog.Nop(),
		connect: func(_ context.Context, preferredID dbconn.ID, _ string) (dbconn.Conn, error) {
			return fakeConn{id: preferredID}, nil
		},
		runVerify: func(
			_ context.Context,
			_ dbconn.OrderedConns,
			_ zerolog.Logger,
			_ inconsistency.Reporter,
			_ utils.FilterConfig,
		) error {
			return errors.New("verify reported checksum mismatch for public.accounts")
		},
	}

	err = runner.Run(context.Background(), request, noopReporter{})
	require.Equal(
		t,
		&operatorError{
			category: "verify_execution",
			code:     "verify_failed",
			message:  "verify execution failed: verify reported checksum mismatch for public.accounts",
			details: []operatorErrorDetail{
				{Reason: "verify reported checksum mismatch for public.accounts"},
			},
		},
		err,
	)
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
