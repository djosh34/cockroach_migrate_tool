package verifyservice

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestLoadConfigSupportsScalarAndExplicitValueCredentials(t *testing.T) {
	scalarConfigPath := writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: verify_source
    password: source:p@ss
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    password: target:p@ss
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`)
	explicitConfigPath := writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      value: verify_source
    password:
      value: source:p@ss
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username:
      value: verify_target
    password:
      value: target:p@ss
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`)

	scalarCfg, err := LoadConfig(scalarConfigPath)
	require.NoError(t, err)
	explicitCfg, err := LoadConfig(explicitConfigPath)
	require.NoError(t, err)

	scalarPair, err := scalarCfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)
	explicitPair, err := explicitCfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)

	scalarSourceConnStr, err := scalarPair.Source.ConnectionString()
	require.NoError(t, err)
	explicitSourceConnStr, err := explicitPair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(t, explicitSourceConnStr, scalarSourceConnStr)
	require.Equal(
		t,
		"postgresql://verify_source:source%3Ap%40ss@source.internal:26257/app?sslmode=disable",
		scalarSourceConnStr,
	)

	scalarDestinationConnStr, err := scalarPair.Destination.ConnectionString()
	require.NoError(t, err)
	explicitDestinationConnStr, err := explicitPair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(t, explicitDestinationConnStr, scalarDestinationConnStr)
	require.Equal(
		t,
		"postgresql://verify_target:target%3Ap%40ss@destination.internal:5432/app?sslmode=disable",
		scalarDestinationConnStr,
	)
}

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
		"postgresql://verify_audit_source:audit-source-pass@source.internal:26257/audit?sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		auditSourceConnStr,
	)
	require.Equal(
		t,
		"postgresql://verify_audit_target:audit-destination-pass@destination.internal:5432/audit?sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
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
		"postgresql://verify_billing_source:billing-source-pass@source.internal:26257/billing?sslcert=%2Fconfig%2Fcerts%2Fsource-client.crt&sslkey=%2Fconfig%2Fcerts%2Fsource-client.key&sslmode=verify-full&sslrootcert=%2Fconfig%2Fcerts%2Fsource-ca.crt",
		billingSourceConnStr,
	)
	require.Equal(
		t,
		"postgresql://verify_billing_target:billing-destination-pass@destination.internal:5432/billing_prod?sslmode=verify-ca&sslrootcert=%2Fconfig%2Fcerts%2Fdestination-ca.crt",
		billingDestinationConnStr,
	)
}

func TestLoadConfigResolvesEnvCredentialsInDefaults(t *testing.T) {
	t.Setenv("VERIFY_SOURCE_USERNAME", "verify_source")
	t.Setenv("VERIFY_SOURCE_PASSWORD", "source:p@ss")
	t.Setenv("VERIFY_DESTINATION_USERNAME", "verify_target")
	t.Setenv("VERIFY_DESTINATION_PASSWORD", "target:p@ss")

	cfg, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      env_ref: VERIFY_SOURCE_USERNAME
    password:
      env_ref: VERIFY_SOURCE_PASSWORD
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username:
      env_ref: VERIFY_DESTINATION_USERNAME
    password:
      env_ref: VERIFY_DESTINATION_PASSWORD
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
	require.NoError(t, err)

	pair, err := cfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)

	sourceConnStr, err := pair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_source:source%3Ap%40ss@source.internal:26257/app?sslmode=disable",
		sourceConnStr,
	)

	destinationConnStr, err := pair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_target:target%3Ap%40ss@destination.internal:5432/app?sslmode=disable",
		destinationConnStr,
	)
}

func TestLoadConfigResolvesSecretFileCredentialsAndTrimsOneTrailingNewline(t *testing.T) {
	sourceUsernamePath := writeTempSecret(t, "source-username", "verify_source\n")
	sourcePasswordPath := writeTempSecret(t, "source-password", "line one\nline two\n")
	destinationUsernamePath := writeTempSecret(t, "destination-username", "verify_target")
	destinationPasswordPath := writeTempSecret(t, "destination-password", "target:p@ss\n")

	cfg, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      secret_file: `+sourceUsernamePath+`
    password:
      secret_file: `+sourcePasswordPath+`
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username:
      secret_file: `+destinationUsernamePath+`
    password:
      secret_file: `+destinationPasswordPath+`
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
	require.NoError(t, err)

	pair, err := cfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)

	sourceConnStr, err := pair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_source:line%20one%0Aline%20two@source.internal:26257/app?sslmode=disable",
		sourceConnStr,
	)

	destinationConnStr, err := pair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_target:target%3Ap%40ss@destination.internal:5432/app?sslmode=disable",
		destinationConnStr,
	)
}

func TestLoadConfigSupportsPerDatabaseCredentialOverridesWithMixedSourceKinds(t *testing.T) {
	t.Setenv("AUDIT_SOURCE_USERNAME", "verify_audit_source")
	auditSourcePasswordPath := writeTempSecret(t, "audit-source-password", "audit-source-pass\n")

	cfg, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: verify_source
    password: source-default-pass
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    password:
      value: destination-default-pass
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
    - name: audit
      source_database: audit
      destination_database: audit_archive
      source:
        username:
          env_ref: AUDIT_SOURCE_USERNAME
        password:
          secret_file: `+auditSourcePasswordPath+`
      destination:
        username:
          value: verify_audit_target
        password: audit-destination-pass
`))
	require.NoError(t, err)

	auditPair, err := cfg.Verify.ResolveDatabase("audit")
	require.NoError(t, err)

	auditSourceConnStr, err := auditPair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_audit_source:audit-source-pass@source.internal:26257/audit?sslmode=disable",
		auditSourceConnStr,
	)

	auditDestinationConnStr, err := auditPair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_audit_target:audit-destination-pass@destination.internal:5432/audit_archive?sslmode=disable",
		auditDestinationConnStr,
	)
}

func TestLoadConfigSupportsNoDefaultsMixedCredentialSources(t *testing.T) {
	t.Setenv("APP_SOURCE_PASSWORD", "app-source-pass")
	destinationUsernamePath := writeTempSecret(t, "app-destination-username", "verify_app_target\n")

	cfg, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  databases:
    - name: app
      source:
        host: source.internal
        port: 26257
        database: app
        username: verify_app_source
        password:
          env_ref: APP_SOURCE_PASSWORD
        sslmode: disable
      destination:
        host: destination.internal
        port: 5432
        database: app_archive
        username:
          secret_file: `+destinationUsernamePath+`
        password:
          value: app-destination-pass
        sslmode: disable
`))
	require.NoError(t, err)

	pair, err := cfg.Verify.ResolveDatabase("app")
	require.NoError(t, err)

	sourceConnStr, err := pair.Source.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_app_source:app-source-pass@source.internal:26257/app?sslmode=disable",
		sourceConnStr,
	)

	destinationConnStr, err := pair.Destination.ConnectionString()
	require.NoError(t, err)
	require.Equal(
		t,
		"postgresql://verify_app_target:app-destination-pass@destination.internal:5432/app_archive?sslmode=disable",
		destinationConnStr,
	)
}

func TestLoadConfigRejectsInvalidCredentialSources(t *testing.T) {
	unreadablePath := filepath.Join(t.TempDir(), "missing-secret")
	emptySecretPath := writeTempSecret(t, "empty-secret", "\n")

	testCases := []struct {
		name         string
		envKey       string
		envValue     string
		configYAML   string
		wantError    string
		wantNoSecret string
	}{
		{
			name: "empty env ref",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      env_ref: ""
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.env_ref must be set",
		},
		{
			name: "unset env ref",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      env_ref: VERIFY_SOURCE_USERNAME
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.env_ref references an unset environment variable",
		},
		{
			name:     "empty env credential",
			envKey:   "VERIFY_SOURCE_USERNAME",
			envValue: "",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      env_ref: VERIFY_SOURCE_USERNAME
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.env_ref resolved to an empty credential",
		},
		{
			name: "empty secret file path",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      secret_file: ""
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.secret_file must be set",
		},
		{
			name: "unreadable secret file",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      secret_file: ` + unreadablePath + `
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.secret_file could not be read",
		},
		{
			name: "empty secret file credential",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      secret_file: ` + emptySecretPath + `
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username.secret_file resolved to an empty credential",
		},
		{
			name: "zero source object",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: {}
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError: "verify.databases[0].source.username must specify exactly one of value, env_ref, or secret_file",
		},
		{
			name: "multiple credential sources",
			configYAML: `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username:
      value: leaked-user
      env_ref: VERIFY_SOURCE_USERNAME
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`,
			wantError:    "verify.databases[0].source.username must not specify more than one of value, env_ref, or secret_file",
			wantNoSecret: "leaked-user",
		},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			if tc.envKey != "" {
				t.Setenv(tc.envKey, tc.envValue)
			}

			_, err := LoadConfig(writeTempConfig(t, tc.configYAML))
			require.Error(t, err)
			require.ErrorContains(t, err, tc.wantError)
			if tc.wantNoSecret != "" {
				require.NotContains(t, err.Error(), tc.wantNoSecret)
			}
		})
	}
}

func TestLoadConfigRejectsLegacyPasswordFileField(t *testing.T) {
	t.Run("default database config", func(t *testing.T) {
		_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: verify_source
    password_file: /tmp/source-password
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
		require.ErrorContains(t, err, "field password_file not found in type verifyservice.DatabaseConfig")
	})

	t.Run("per database override", func(t *testing.T) {
		_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
      destination:
        password_file: /tmp/destination-password
`))
		require.ErrorContains(t, err, "field password_file not found in type verifyservice.DatabaseConfig")
	})
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
    username: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
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
    username: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
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
    username: verify_target
    sslmode: disable
  databases:
    - name: app
      source_database: app
      destination_database: app
`))
	require.ErrorContains(t, err, "verify.databases[0].source.username must be set")
}

func TestLoadConfigRejectsInvalidTLSEffectiveConfig(t *testing.T) {
	_, err := LoadConfig(writeTempConfig(t, `listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    username: verify_source
    sslmode: verify-full
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
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
    username: verify_source
    sslmode: disable
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
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

func writeTempSecret(t *testing.T, name string, content string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), name)
	require.NoError(t, os.WriteFile(path, []byte(content), 0o600))
	return path
}
