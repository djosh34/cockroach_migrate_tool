package verifyservice

import (
	"net"
	"net/url"
	"strconv"

	"github.com/cockroachdb/errors"
)

type ResolvedDatabasePair struct {
	Name        string
	Source      ResolvedConnection
	Destination ResolvedConnection
}

type ResolvedConnection struct {
	Host         string
	Port         int
	Database     string
	User         string
	PasswordFile string
	SSLMode      string
	TLS          DatabaseTLSConfig
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
	resolved := ResolvedConnection{
		Host:         firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.Host }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.Host })),
		Port:         firstNonZero(databaseConfigIntValue(override, func(cfg DatabaseConfig) int { return cfg.Port }), databaseConfigIntValue(defaults, func(cfg DatabaseConfig) int { return cfg.Port })),
		Database:     firstNonEmpty(database, databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.Database })),
		User:         firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.User }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.User })),
		PasswordFile: firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.PasswordFile }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.PasswordFile })),
		SSLMode:      firstNonEmpty(databaseConfigValue(override, func(cfg DatabaseConfig) string { return cfg.SSLMode }), databaseConfigValue(defaults, func(cfg DatabaseConfig) string { return cfg.SSLMode })),
		TLS: mergeTLS(
			databaseTLSConfig(defaults),
			databaseTLSConfig(override),
		),
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
		{field: "user", value: cfg.User},
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
	if cfg.PasswordFile != "" {
		query.Set("passfile", cfg.PasswordFile)
	}
	if cfg.TLS.CACertPath != "" {
		query.Set("sslrootcert", cfg.TLS.CACertPath)
	}
	if cfg.TLS.ClientCertPath != "" {
		query.Set("sslcert", cfg.TLS.ClientCertPath)
		query.Set("sslkey", cfg.TLS.ClientKeyPath)
	}

	return (&url.URL{
		Scheme:   "postgresql",
		User:     url.User(cfg.User),
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
