package verifyservice

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestLoadConfigSupportsStructuredMultiDatabaseDefaults(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-multi-database-defaults.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput)

	appPair, err := cfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)
	appSourceConnStr, err := appPair.Source.ConnectionString()
	require.NoError(t, err)
	appDestinationConnStr, err := appPair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_source@source.internal:26257/app?sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		appSourceConnStr,
	)
	require.Equal(
		t,
		"postgresql://verify_target@destination.internal:5432/app?sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
		appDestinationConnStr,
	)

	supportPair, err := cfg.Verify.ResolveDatabase("support")
	require.NoError(t, err)
	supportDestinationConnStr, err := supportPair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_target@destination.internal:5432/support_archive?sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
		supportDestinationConnStr,
	)
}

func TestLoadConfigSupportsPerDatabaseCredentialOverrides(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-multi-database-override.yml"))
	require.NoError(t, err)

	auditPair, err := cfg.Verify.ResolveDatabase("audit")
	require.NoError(t, err)

	auditSourceConnStr, err := auditPair.Source.ConnectionString()
	require.NoError(t, err)
	auditDestinationConnStr, err := auditPair.Destination.ConnectionString()
	require.NoError(t, err)

	require.Equal(
		t,
		"postgresql://verify_audit_source@source.internal:26257/audit?passfile=%2Fconfig%2Fsecrets%2Faudit-source-password&sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		auditSourceConnStr,
	)
	require.Equal(
		t,
		"postgresql://verify_audit_target@destination.internal:5432/audit?passfile=%2Fconfig%2Fsecrets%2Faudit-destination-password&sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
		auditDestinationConnStr,
	)
}

func TestLoadConfigSupportsFullySpecifiedPerDatabaseConnectionsWithoutDefaults(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-multi-database-no-defaults.yml"))
	require.NoError(t, err)

	billingPair, err := cfg.Verify.ResolveDatabase("billing")
	require.NoError(t, err)

	billingSourceConnStr, err := billingPair.Source.ConnectionString()
	require.NoError(t, err)
	billingDestinationConnStr, err := billingPair.Destination.ConnectionString()
	require.NoError(t, err)

	require.Equal(
		t,
		"postgresql://verify_billing_source@source.internal:26257/billing?passfile=%2Fconfig%2Fsecrets%2Fbilling-source-password&sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		billingSourceConnStr,
	)
	require.Equal(
		t,
		"postgresql://verify_billing_target@destination.internal:5432/billing_prod?passfile=%2Fconfig%2Fsecrets%2Fbilling-destination-password&sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
		billingDestinationConnStr,
	)
}

func TestLoadConfigRejectsRemovedFlatTLSKnobs(t *testing.T) {
	t.Run("listener transport block is rejected", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-obsolete-listener-transport.yml"))
		require.ErrorContains(t, err, "field transport not found in type verifyservice.ListenerConfig")
	})

	t.Run("database flat tls fields are rejected", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-obsolete-database-tls.yml"))
		require.ErrorContains(t, err, "field ca_cert_path not found in type verifyservice.DatabaseConfig")
	})
}

func TestLoadConfigRejectsDuplicateDatabaseNames(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
    - name: app
      source_database: billing
      destination_database: billing
`))
	require.ErrorContains(t, err, `verify.databases[1].name duplicates configured database "app"`)
}

func TestLoadConfigRejectsScalarDatabaseEntries(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: disable
  databases:
    - app
`))
	require.ErrorContains(t, err, "verify.databases[0] must be a mapping object")
}

func TestLoadConfigRejectsMissingInheritedFields(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
	require.ErrorContains(t, err, "verify.databases[0].source.user must be set")
}

func TestLoadConfigRejectsInvalidTLSEffectiveConfig(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: verify-full
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
	require.ErrorContains(t, err, "verify.databases[0].source.tls.ca_cert_path must be set when verify.databases[0].source.sslmode verifies the server certificate")
}

func TestLoadConfigRejectsUnsupportedStructuredFields(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
      source:
        params:
          application_name: verify
`))
	require.ErrorContains(t, err, "field params not found in type verifyservice.DatabaseConfig")
}

func TestLoadConfigSupportsPasswordlessClientCertificates(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-passwordless-client-cert.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput)

	pair, err := cfg.Verify.ResolveDatabase("")
	require.NoError(t, err)

	sourceConnStr, err := pair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_source@source.internal:5432/appdb?sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		sourceConnStr,
	)
}

func TestLoadConfigRejectsUnpairedClientCertificateMaterial(t *testing.T) {
	t.Run("client cert without key", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-source-client-cert-without-key.yml"))
		require.ErrorContains(t, err, "verify.databases[0].source.tls.client_cert_path and verify.databases[0].source.tls.client_key_path must both be set")
	})

	t.Run("client key without cert", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-destination-client-key-without-cert.yml"))
		require.ErrorContains(t, err, "verify.databases[0].destination.tls.client_cert_path and verify.databases[0].destination.tls.client_key_path must both be set")
	})
}

func TestConfigValidateDefaultsRawTableOutputToDisabled(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-passwordless-client-cert.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput)
}

func TestLoadConfigSupportsOperatorChosenListenerModes(t *testing.T) {
	t.Run("http listener is accepted", func(t *testing.T) {
		cfg, err := LoadConfig(filepath.Join("testdata", "valid-http-listener.yml"))
		require.NoError(t, err)
		require.Equal(t, "0.0.0.0:8080", cfg.Listener.BindAddr)
		require.Equal(t, "http", cfg.Listener.Mode())
	})

	t.Run("https listener without mtls is accepted", func(t *testing.T) {
		cfg, err := LoadConfig(filepath.Join("testdata", "valid-https-server-tls.yml"))
		require.NoError(t, err)
		require.NotNil(t, cfg.Listener.TLS)
		require.Equal(t, "/config/certs/server.crt", cfg.Listener.TLS.CertPath)
		require.Equal(t, "https", cfg.Listener.Mode())
	})

	t.Run("https listener with mtls is accepted", func(t *testing.T) {
		cfg, err := LoadConfig(filepath.Join("testdata", "valid-https-mtls.yml"))
		require.NoError(t, err)
		require.NotNil(t, cfg.Listener.TLS)
		require.Equal(t, "/config/certs/client-ca.crt", cfg.Listener.TLS.ClientCAPath)
		require.Equal(t, "https+mtls", cfg.Listener.Mode())
	})

	t.Run("https requires cert and key", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-https-without-server-cert.yml"))
		require.ErrorContains(t, err, "listener.tls.cert_path and listener.tls.key_path must both be set when listener.tls is configured")
	})
}

func writeTempConfig(t *testing.T, content string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "verify-service.yml")
	require.NoError(t, os.WriteFile(path, []byte(content), 0o600))
	return path
}
