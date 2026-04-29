package verifyservice

import (
	"net"
	"net/url"
	"os"
	"strconv"
	"strings"

	"github.com/cockroachdb/errors"
)

type ResolvedDatabasePair struct {
	Name        string
	Source      ResolvedConnection
	Destination ResolvedConnection
}

type ResolvedConnection struct {
	Host     string
	Port     int
	Database string
	Username string
	Password string
	SSLMode  string
	TLS      DatabaseTLSConfig
}

type effectiveDatabaseConfig struct {
	Host     string
	Port     int
	Database string
	Username CredentialValue
	Password CredentialValue
	SSLMode  string
	TLS      DatabaseTLSConfig
}

func (cfg VerifyConfig) ResolveDatabase(name string) (ResolvedDatabasePair, error) {
	if name == "" {
		if len(cfg.Databases) == 1 {
			return cfg.resolveDatabaseAt("verify.databases[0]", cfg.Databases[0])
		}
		return ResolvedDatabasePair{}, errors.New("database selection is required when multiple databases are configured")
	}

	for index, database := range cfg.Databases {
		if database.Name == name {
			return cfg.resolveDatabaseAt(databasePath(index), database)
		}
	}
	return ResolvedDatabasePair{}, errors.Newf("configured database %q was not found", name)
}

func (cfg VerifyConfig) ResolveAllDatabases() ([]ResolvedDatabasePair, error) {
	resolved := make([]ResolvedDatabasePair, 0, len(cfg.Databases))
	for index, database := range cfg.Databases {
		pair, err := cfg.resolveDatabaseAt(databasePath(index), database)
		if err != nil {
			return nil, err
		}
		resolved = append(resolved, pair)
	}
	return resolved, nil
}

func (cfg VerifyConfig) resolveDatabaseAt(path string, database DatabaseMappingConfig) (ResolvedDatabasePair, error) {
	sourceDatabase := firstNonEmpty(database.SourceDatabase, databaseConfigValue(database.Source, func(cfg DatabaseConfig) string { return cfg.Database }))
	source, err := resolveConnection(path+".source", cfg.Source, database.Source, sourceDatabase)
	if err != nil {
		return ResolvedDatabasePair{}, err
	}

	destinationDatabase := firstNonEmpty(database.DestinationDatabase, databaseConfigValue(database.Destination, func(cfg DatabaseConfig) string { return cfg.Database }))
	destination, err := resolveConnection(path+".destination", cfg.Destination, database.Destination, destinationDatabase)
	if err != nil {
		return ResolvedDatabasePair{}, err
	}

	return ResolvedDatabasePair{
		Name:        database.Name,
		Source:      source,
		Destination: destination,
	}, nil
}

func resolveConnection(path string, defaults *DatabaseConfig, override *DatabaseConfig, database string) (ResolvedConnection, error) {
	effective := effectiveDatabaseConfig{
		Host:     firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.Host }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.Host })),
		Port:     firstNonZero(databaseConfigIntValue(override, func(cfg DatabaseConfig) int { return cfg.Port }), databaseConfigIntValue(defaults, func(cfg DatabaseConfig) int { return cfg.Port })),
		Database: firstNonEmpty(database, databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.Database })),
		Username: mergeCredential(
			databaseConfigCredential(defaults, func(cfg DatabaseConfig) CredentialValue { return cfg.Username }),
			databaseConfigCredential(override, func(cfg DatabaseConfig) CredentialValue { return cfg.Username }),
		),
		Password: mergeCredential(
			databaseConfigCredential(defaults, func(cfg DatabaseConfig) CredentialValue { return cfg.Password }),
			databaseConfigCredential(override, func(cfg DatabaseConfig) CredentialValue { return cfg.Password }),
		),
		SSLMode: firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.SSLMode }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.SSLMode })),
		TLS: mergeTLS(
			databaseTLSConfig(defaults),
			databaseTLSConfig(override),
		),
	}

	username, err := resolveRequiredCredential(path+".username", effective.Username)
	if err != nil {
		return ResolvedConnection{}, err
	}
	password, err := resolveOptionalCredential(path+".password", effective.Password)
	if err != nil {
		return ResolvedConnection{}, err
	}

	resolved := ResolvedConnection{
		Host:     effective.Host,
		Port:     effective.Port,
		Database: effective.Database,
		Username: username,
		Password: password,
		SSLMode:  effective.SSLMode,
		TLS:      effective.TLS,
	}
	if err := resolved.validate(path); err != nil {
		return ResolvedConnection{}, err
	}
	return resolved, nil
}

func (cfg ResolvedConnection) validate(path string) error {
	for _, required := range []struct {
		field string
		value string
	}{
		{field: "host", value: cfg.Host},
		{field: "database", value: cfg.Database},
		{field: "username", value: cfg.Username},
		{field: "sslmode", value: cfg.SSLMode},
	} {
		if required.value == "" {
			return errors.Newf("%s.%s must be set", path, required.field)
		}
	}
	if cfg.Port == 0 {
		return errors.Newf("%s.port must be set", path)
	}
	if sslModeRequiresServerVerification(cfg.SSLMode) && cfg.TLS.CACertPath == "" {
		return errors.Newf("%s.tls.ca_cert_path must be set when %s.sslmode verifies the server certificate", path, path)
	}

	hasClientCert := cfg.TLS.ClientCertPath != ""
	hasClientKey := cfg.TLS.ClientKeyPath != ""
	if hasClientCert != hasClientKey {
		return errors.Newf("%s.tls.client_cert_path and %s.tls.client_key_path must both be set", path, path)
	}
	return nil
}

func (cfg ResolvedConnection) ConnectionString() (string, error) {
	query := url.Values{}
	query.Set("sslmode", cfg.SSLMode)
	if cfg.TLS.CACertPath != "" {
		query.Set("sslrootcert", cfg.TLS.CACertPath)
	}
	if cfg.TLS.ClientCertPath != "" {
		query.Set("sslcert", cfg.TLS.ClientCertPath)
		query.Set("sslkey", cfg.TLS.ClientKeyPath)
	}

	userInfo := url.User(cfg.Username)
	if cfg.Password != "" {
		userInfo = url.UserPassword(cfg.Username, cfg.Password)
	}

	return (&url.URL{
		Scheme:   "postgresql",
		User:     userInfo,
		Host:     net.JoinHostPort(cfg.Host, strconv.Itoa(cfg.Port)),
		Path:     "/" + cfg.Database,
		RawQuery: query.Encode(),
	}).String(), nil
}

func sslModeRequiresServerVerification(mode string) bool {
	switch mode {
	case "verify-ca", "verify-full":
		return true
	default:
		return false
	}
}

func mergeTLS(defaults DatabaseTLSConfig, override DatabaseTLSConfig) DatabaseTLSConfig {
	return DatabaseTLSConfig{
		CACertPath:     firstNonEmpty(override.CACertPath, defaults.CACertPath),
		ClientCertPath: firstNonEmpty(override.ClientCertPath, defaults.ClientCertPath),
		ClientKeyPath:  firstNonEmpty(override.ClientKeyPath, defaults.ClientKeyPath),
	}
}

func databaseTLSConfig(cfg *DatabaseConfig) DatabaseTLSConfig {
	if cfg == nil || cfg.TLS == nil {
		return DatabaseTLSConfig{}
	}
	return *cfg.TLS
}

func databaseConfigValue(cfg *DatabaseConfig, getter func(DatabaseConfig) string) string {
	if cfg == nil {
		return ""
	}
	return getter(*cfg)
}

func databaseConfigIntValue(cfg *DatabaseConfig, getter func(DatabaseConfig) int) int {
	if cfg == nil {
		return 0
	}
	return getter(*cfg)
}

func databaseConfigCredential(cfg *DatabaseConfig, getter func(DatabaseConfig) CredentialValue) CredentialValue {
	if cfg == nil {
		return CredentialValue{}
	}
	return getter(*cfg)
}

func databasePath(index int) string {
	return "verify.databases[" + strconv.Itoa(index) + "]"
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if value != "" {
			return value
		}
	}
	return ""
}

func firstNonZero(values ...int) int {
	for _, value := range values {
		if value != 0 {
			return value
		}
	}
	return 0
}

func mergeCredential(defaults CredentialValue, override CredentialValue) CredentialValue {
	if override.declared() {
		return override
	}
	return defaults
}

func resolveRequiredCredential(path string, value CredentialValue) (string, error) {
	if !value.declared() {
		return "", newCredentialConfigError(path, "must be set")
	}
	return resolveCredential(path, value)
}

func resolveOptionalCredential(path string, value CredentialValue) (string, error) {
	if !value.declared() {
		return "", nil
	}
	return resolveCredential(path, value)
}

func resolveCredential(path string, value CredentialValue) (string, error) {
	if value.sourceCount() == 0 {
		return "", newCredentialConfigError(path, "must specify exactly one of value, env_ref, or secret_file")
	}
	if value.sourceCount() > 1 {
		return "", newCredentialConfigError(path, "must not specify more than one of value, env_ref, or secret_file")
	}

	switch {
	case value.valueSet || value.Value != "":
		if value.Value == "" {
			return "", newCredentialConfigError(path+".value", "must not be empty")
		}
		return value.Value, nil
	case value.envRefSet || value.EnvRef != "":
		if value.EnvRef == "" {
			return "", newCredentialConfigError(path+".env_ref", "must be set")
		}
		resolved, ok := os.LookupEnv(value.EnvRef)
		if !ok {
			return "", newCredentialConfigError(path+".env_ref", "references an unset environment variable")
		}
		if resolved == "" {
			return "", newCredentialConfigError(path+".env_ref", "resolved to an empty credential")
		}
		return resolved, nil
	case value.secretFileSet || value.SecretFile != "":
		if value.SecretFile == "" {
			return "", newCredentialConfigError(path+".secret_file", "must be set")
		}
		content, err := os.ReadFile(value.SecretFile)
		if err != nil {
			return "", newCredentialConfigError(path+".secret_file", "could not be read: "+err.Error())
		}
		resolved := trimTrailingCredentialNewline(string(content))
		if resolved == "" {
			return "", newCredentialConfigError(path+".secret_file", "resolved to an empty credential")
		}
		return resolved, nil
	default:
		return "", newCredentialConfigError(path, "must specify exactly one of value, env_ref, or secret_file")
	}
}

func trimTrailingCredentialNewline(value string) string {
	if strings.HasSuffix(value, "\r\n") {
		return strings.TrimSuffix(value, "\r\n")
	}
	return strings.TrimSuffix(value, "\n")
}

func newCredentialConfigError(field string, reason string) error {
	return newOperatorError(
		"config",
		"invalid_config",
		"verify-service config is invalid",
		operatorErrorDetail{
			Field:  field,
			Reason: reason,
		},
	)
}
