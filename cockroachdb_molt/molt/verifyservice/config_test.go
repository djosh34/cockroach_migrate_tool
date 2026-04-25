package verifyservice

import (
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

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

func TestLoadConfigSupportsPasswordlessClientCertificates(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-passwordless-client-cert.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput)

	sourceConnStr, err := cfg.Verify.Source.ConnectionString()
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
		require.ErrorContains(t, err, "verify.source.tls.client_cert_path and verify.source.tls.client_key_path must both be set")
	})

	t.Run("client key without cert", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-destination-client-key-without-cert.yml"))
		require.ErrorContains(t, err, "verify.destination.tls.client_cert_path and verify.destination.tls.client_key_path must both be set")
	})
}

func TestConfigValidateDefaultsRawTableOutputToDisabled(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-passwordless-client-cert.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput)
}

func TestLoadConfigRejectsNonPostgresDatabaseSchemes(t *testing.T) {
	t.Run("mysql source", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-mysql-source-url.yml"))
		require.ErrorContains(t, err, "verify.source.url must use postgres or postgresql scheme")
	})

	t.Run("oracle destination", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-oracle-destination-url.yml"))
		require.ErrorContains(t, err, "verify.destination.url must use postgres or postgresql scheme")
	})
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
