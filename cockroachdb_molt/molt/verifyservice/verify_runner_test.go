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

func TestVerifyRunnerUsesResolvedConnectionStringsAndGlobFilters(t *testing.T) {
	t.Parallel()

	cfg := singleDatabaseVerifyConfig(
		DatabaseConfig{
			Host:     "source-db",
			Port:     26257,
			Database: "source_db",
			Username: CredentialValue{Value: "source-user"},
			SSLMode:  "verify-full",
			TLS: &DatabaseTLSConfig{
				CACertPath:     "/etc/source-ca.pem",
				ClientCertPath: "/etc/source-client.pem",
				ClientKeyPath:  "/etc/source-client.key",
			},
		},
		DatabaseConfig{
			Host:     "target-db",
			Port:     26257,
			Database: "target_db",
			Username: CredentialValue{Value: "target-user"},
			SSLMode:  "verify-ca",
			TLS: &DatabaseTLSConfig{
				CACertPath: "/etc/target-ca.pem",
			},
		},
	)
	request := compileSingleRunRequest(t, cfg, JobRequest{
		DefaultSchemaMatch: matchExpression{"public"},
		DefaultTableMatch:  matchExpression{"accounts*", "orders"},
	})

	sourceConnStr, err := request.ResolvedDatabase.Source.ConnectionString()
	require.NoError(t, err)
	destinationConnStr, err := request.ResolvedDatabase.Destination.ConnectionString()
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
		SchemaFilter: "^(?:public)$",
		TableFilter:  "^(?:accounts.*|orders)$",
	}, gotFilter)
}

func TestVerifyRunnerClassifiesSourceConnectionFailures(t *testing.T) {
	t.Parallel()

	request := compileSingleRunRequest(t, singleDatabaseVerifyConfig(
		DatabaseConfig{
			Host:     "source-db",
			Port:     26257,
			Database: "source_db",
			Username: CredentialValue{Value: "verify_source"},
			SSLMode:  "disable",
		},
		DatabaseConfig{
			Host:     "target-db",
			Port:     26257,
			Database: "target_db",
			Username: CredentialValue{Value: "verify_target"},
			SSLMode:  "disable",
		},
	), JobRequest{})

	runner := VerifyRunner{
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

	err := runner.Run(context.Background(), request, noopReporter{})
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

	request := compileSingleRunRequest(t, singleDatabaseVerifyConfig(
		DatabaseConfig{
			Host:     "source-db",
			Port:     26257,
			Database: "source_db",
			Username: CredentialValue{Value: "verify_source"},
			SSLMode:  "disable",
		},
		DatabaseConfig{
			Host:     "target-db",
			Port:     26257,
			Database: "target_db",
			Username: CredentialValue{Value: "verify_target"},
			SSLMode:  "disable",
		},
	), JobRequest{})

	runner := VerifyRunner{
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

	err := runner.Run(context.Background(), request, noopReporter{})
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

	request := compileSingleRunRequest(t, singleDatabaseVerifyConfig(
		DatabaseConfig{
			Host:     "source-db",
			Port:     26257,
			Database: "source_db",
			Username: CredentialValue{Value: "verify_source"},
			SSLMode:  "disable",
		},
		DatabaseConfig{
			Host:     "target-db",
			Port:     26257,
			Database: "target_db",
			Username: CredentialValue{Value: "verify_target"},
			SSLMode:  "disable",
		},
	), JobRequest{})

	runner := VerifyRunner{
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

	err := runner.Run(context.Background(), request, noopReporter{})
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

func TestResolvedRunRequestUsesRequestedConfiguredDatabase(t *testing.T) {
	t.Parallel()

	cfg := Config{
		Verify: VerifyConfig{
			Source: &DatabaseConfig{
				Host:     "source-db",
				Port:     26257,
				Username: CredentialValue{Value: "verify_source"},
				SSLMode:  "disable",
			},
			Destination: &DatabaseConfig{
				Host:     "target-db",
				Port:     5432,
				Username: CredentialValue{Value: "verify_target"},
				SSLMode:  "disable",
			},
			Databases: []DatabaseMappingConfig{
				{
					Name:                "app",
					SourceDatabase:      "app",
					DestinationDatabase: "app",
				},
				{
					Name:                "billing",
					SourceDatabase:      "billing",
					DestinationDatabase: "billing_archive",
				},
			},
		},
	}

	request := compileSingleRunRequest(t, cfg, JobRequest{
		Databases: databaseRequestValue{{DatabaseMatch: "billing"}},
	})

	require.Equal(t, "billing", request.DatabaseName)
	require.Equal(t, "billing", request.ResolvedDatabase.Source.Database)
	require.Equal(t, "billing_archive", request.ResolvedDatabase.Destination.Database)
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

func singleDatabaseVerifyConfig(source DatabaseConfig, destination DatabaseConfig) Config {
	return Config{
		Verify: VerifyConfig{
			Source:      &source,
			Destination: &destination,
			Databases: []DatabaseMappingConfig{
				{
					Name:                "default",
					SourceDatabase:      source.Database,
					DestinationDatabase: destination.Database,
				},
			},
		},
	}
}

func compileSingleRunRequest(t *testing.T, cfg Config, request JobRequest) RunRequest {
	t.Helper()

	normalized, err := request.Compile()
	require.NoError(t, err)

	plan, err := normalized.Resolve(cfg.Verify)
	require.NoError(t, err)
	require.Len(t, plan.Databases, 1)

	runRequest, err := plan.Databases[0].RunRequest()
	require.NoError(t, err)
	return runRequest
}
