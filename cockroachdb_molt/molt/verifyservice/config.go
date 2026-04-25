package verifyservice

import (
	"bytes"
	"net/url"
	"os"

	"github.com/cockroachdb/errors"
	"gopkg.in/yaml.v3"
)

type Config struct {
	Listener ListenerConfig `yaml:"listener"`
	Verify   VerifyConfig   `yaml:"verify"`
}

type ListenerConfig struct {
	BindAddr string             `yaml:"bind_addr"`
	TLS      *ListenerTLSConfig `yaml:"tls,omitempty"`
}

type ListenerTLSConfig struct {
	CertPath     string `yaml:"cert_path"`
	KeyPath      string `yaml:"key_path"`
	ClientCAPath string `yaml:"client_ca_path,omitempty"`
}

type VerifyConfig struct {
	Source         DatabaseConfig `yaml:"source"`
	Destination    DatabaseConfig `yaml:"destination"`
	RawTableOutput bool           `yaml:"raw_table_output"`
}

type DatabaseConfig struct {
	URL string             `yaml:"url"`
	TLS *DatabaseTLSConfig `yaml:"tls,omitempty"`
}

type DatabaseTLSConfig struct {
	CACertPath     string `yaml:"ca_cert_path,omitempty"`
	ClientCertPath string `yaml:"client_cert_path,omitempty"`
	ClientKeyPath  string `yaml:"client_key_path,omitempty"`
}

func LoadConfig(path string) (Config, error) {
	var cfg Config
	content, err := os.ReadFile(path)
	if err != nil {
		return Config{}, newOperatorError(
			"config",
			"config_read_failed",
			"verify-service config could not be read",
			operatorErrorDetail{Reason: err.Error()},
		)
	}

	decoder := yaml.NewDecoder(bytes.NewReader(content))
	decoder.KnownFields(true)
	if err := decoder.Decode(&cfg); err != nil {
		return Config{}, newOperatorError(
			"config",
			"invalid_config",
			"verify-service config is invalid",
			operatorErrorDetail{Reason: err.Error()},
		)
	}
	if err := cfg.Validate(); err != nil {
		return Config{}, newOperatorError(
			"config",
			"invalid_config",
			"verify-service config is invalid",
			operatorErrorDetail{Reason: err.Error()},
		)
	}
	return cfg, nil
}

func (cfg Config) Validate() error {
	if err := cfg.Listener.validate(); err != nil {
		return err
	}
	if err := cfg.Verify.Source.validate("verify.source"); err != nil {
		return err
	}
	if err := cfg.Verify.Destination.validate("verify.destination"); err != nil {
		return err
	}
	return nil
}

func (cfg ListenerConfig) validate() error {
	if cfg.BindAddr == "" {
		return errors.New("listener.bind_addr must be set")
	}
	if cfg.TLS == nil {
		return nil
	}
	if cfg.TLS.CertPath == "" || cfg.TLS.KeyPath == "" {
		return errors.New("listener.tls.cert_path and listener.tls.key_path must both be set when listener.tls is configured")
	}
	return nil
}

func (cfg ListenerConfig) Mode() string {
	if cfg.TLS == nil {
		return "http"
	}
	if cfg.TLS.ClientCAPath != "" {
		return "https+mtls"
	}
	return "https"
}

func (cfg DatabaseConfig) validate(path string) error {
	if err := validatePostgresScheme(cfg.URL); err != nil {
		return errors.Newf("%s.url %s", path, err.Error())
	}
	tlsPath := path + ".tls"
	if sslModeRequiresServerVerification(cfg.SSLMode()) && cfg.tlsConfig().CACertPath == "" {
		return errors.Newf("%s.ca_cert_path must be set when %s.url sslmode verifies the server certificate", tlsPath, path)
	}

	hasClientCert := cfg.tlsConfig().ClientCertPath != ""
	hasClientKey := cfg.tlsConfig().ClientKeyPath != ""
	if hasClientCert != hasClientKey {
		return errors.Newf("%s.client_cert_path and %s.client_key_path must both be set", tlsPath, tlsPath)
	}
	return nil
}

func (cfg DatabaseConfig) SSLMode() string {
	parsed, err := url.Parse(cfg.URL)
	if err != nil {
		return ""
	}
	return parsed.Query().Get("sslmode")
}

func sslModeRequiresServerVerification(mode string) bool {
	switch mode {
	case "verify-ca", "verify-full":
		return true
	default:
		return false
	}
}

func validatePostgresScheme(rawURL string) error {
	parsed, err := url.Parse(rawURL)
	if err != nil {
		return errors.Wrap(err, "must be a valid URL")
	}
	switch parsed.Scheme {
	case "postgres", "postgresql":
		return nil
	default:
		return errors.New("must use postgres or postgresql scheme")
	}
}

func (cfg DatabaseConfig) ConnectionString() (string, error) {
	parsed, err := url.Parse(cfg.URL)
	if err != nil {
		return "", errors.Wrap(err, "parse database url")
	}
	query := parsed.Query()
	if cfg.tlsConfig().CACertPath != "" {
		query.Set("sslrootcert", cfg.tlsConfig().CACertPath)
	}
	if cfg.tlsConfig().ClientCertPath != "" {
		query.Set("sslcert", cfg.tlsConfig().ClientCertPath)
		query.Set("sslkey", cfg.tlsConfig().ClientKeyPath)
	}
	parsed.RawQuery = query.Encode()
	return parsed.String(), nil
}

func (cfg DatabaseConfig) tlsConfig() DatabaseTLSConfig {
	if cfg.TLS == nil {
		return DatabaseTLSConfig{}
	}
	return *cfg.TLS
}
