package verifyservice

import (
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestLoadConfigRequiresExplicitVerifyModes(t *testing.T) {
	t.Run("missing source mode", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-missing-source-mode.yml"))
		require.ErrorContains(t, err, "verify.source.tls.mode must be one of: verify-full, verify-ca")
	})

	t.Run("invalid destination mode", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-destination-mode.yml"))
		require.ErrorContains(t, err, "verify.destination.tls.mode must be one of: verify-full, verify-ca")
	})
}

func TestLoadConfigSupportsPasswordlessClientCertificates(t *testing.T) {
	cfg, err := LoadConfig(filepath.Join("testdata", "valid-passwordless-client-cert.yml"))
	require.NoError(t, err)
	require.False(t, cfg.Verify.RawTableOutput.Enabled)

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
	require.False(t, cfg.Verify.RawTableOutput.Enabled)
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

func TestLoadConfigValidatesListenerProtectionModes(t *testing.T) {
	t.Run("http listener is rejected", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-http-listener.yml"))
		require.ErrorContains(t, err, "listener.transport.mode must be https")
	})

	t.Run("https requires cert and key", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-https-without-server-cert.yml"))
		require.ErrorContains(t, err, "listener.tls.cert_path and listener.tls.key_path must both be set for https")
	})

	t.Run("https listener requires mtls", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-https-without-client-auth.yml"))
		require.ErrorContains(t, err, "listener.tls.client_auth.mode must be mtls")
	})

	t.Run("http listener is rejected even with mtls configured", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-http-mtls.yml"))
		require.ErrorContains(t, err, "listener.transport.mode must be https")
	})

	t.Run("mtls requires client ca", func(t *testing.T) {
		_, err := LoadConfig(filepath.Join("testdata", "invalid-mtls-without-client-ca.yml"))
		require.ErrorContains(t, err, "listener.tls.client_auth.client_ca_path must be set when listener.tls.client_auth.mode is mtls")
	})
}
