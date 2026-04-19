package verifyservice

import (
	"net/url"
	"os"

	"github.com/cockroachdb/errors"
	"gopkg.in/yaml.v3"
)

type DBTLSMode string

const (
	DBTLSModeVerifyFull DBTLSMode = "verify-full"
	DBTLSModeVerifyCA   DBTLSMode = "verify-ca"
)

type ListenerTransportMode string

const (
	ListenerTransportModeHTTPS ListenerTransportMode = "https"
)

type ListenerClientAuthMode string

const (
	ListenerClientAuthModeMTLS ListenerClientAuthMode = "mtls"
)

type Config struct {
	Listener ListenerConfig `yaml:"listener"`
	Verify   VerifyConfig   `yaml:"verify"`
}

type ListenerConfig struct {
	BindAddr  string                  `yaml:"bind_addr"`
	Transport ListenerTransportConfig `yaml:"transport"`
	TLS       ListenerTLSConfig       `yaml:"tls"`
}

type ListenerTransportConfig struct {
	Mode ListenerTransportMode `yaml:"mode"`
}

type ListenerTLSConfig struct {
	CertPath   string                   `yaml:"cert_path"`
	KeyPath    string                   `yaml:"key_path"`
	ClientAuth ListenerClientAuthConfig `yaml:"client_auth"`
}

type ListenerClientAuthConfig struct {
	Mode         ListenerClientAuthMode `yaml:"mode"`
	ClientCAPath string                 `yaml:"client_ca_path"`
}

type VerifyConfig struct {
	Source      DatabaseConfig `yaml:"source"`
	Destination DatabaseConfig `yaml:"destination"`
}

type DatabaseConfig struct {
	URL string            `yaml:"url"`
	TLS DatabaseTLSConfig `yaml:"tls"`
}

type DatabaseTLSConfig struct {
	Mode           DBTLSMode `yaml:"mode"`
	CACertPath     string    `yaml:"ca_cert_path"`
	ClientCertPath string    `yaml:"client_cert_path"`
	ClientKeyPath  string    `yaml:"client_key_path"`
}

func LoadConfig(path string) (Config, error) {
	var cfg Config
	content, err := os.ReadFile(path)
	if err != nil {
		return Config{}, err
	}
	if err := yaml.Unmarshal(content, &cfg); err != nil {
		return Config{}, err
	}
	if err := cfg.Validate(); err != nil {
		return Config{}, err
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

func (cfg DatabaseConfig) validate(path string) error {
	if err := cfg.TLS.Mode.Validate(); err != nil {
		return errors.Newf("%s.tls.mode %s", path, err.Error())
	}
	if cfg.TLS.CACertPath == "" {
		return errors.Newf("%s.tls.ca_cert_path must be set", path)
	}
	hasClientCert := cfg.TLS.ClientCertPath != ""
	hasClientKey := cfg.TLS.ClientKeyPath != ""
	if hasClientCert != hasClientKey {
		return errors.Newf("%s.tls.client_cert_path and %s.tls.client_key_path must both be set", path, path)
	}
	return nil
}

func (mode DBTLSMode) Validate() error {
	switch mode {
	case DBTLSModeVerifyFull, DBTLSModeVerifyCA:
		return nil
	default:
		return errors.New("must be one of: verify-full, verify-ca")
	}
}

func (cfg DatabaseConfig) ConnectionString() (string, error) {
	parsed, err := url.Parse(cfg.URL)
	if err != nil {
		return "", errors.Wrap(err, "parse database url")
	}
	query := parsed.Query()
	query.Set("sslmode", string(cfg.TLS.Mode))
	query.Set("sslrootcert", cfg.TLS.CACertPath)
	if cfg.TLS.ClientCertPath != "" {
		query.Set("sslcert", cfg.TLS.ClientCertPath)
		query.Set("sslkey", cfg.TLS.ClientKeyPath)
	}
	parsed.RawQuery = query.Encode()
	return parsed.String(), nil
}

func (cfg ListenerConfig) validate() error {
	if err := cfg.Transport.Mode.Validate(); err != nil {
		return errors.Newf("listener.transport.mode %s", err.Error())
	}
	if err := cfg.TLS.ClientAuth.Mode.Validate(); err != nil {
		return errors.Newf("listener.tls.client_auth.mode %s", err.Error())
	}
	if cfg.TLS.CertPath == "" || cfg.TLS.KeyPath == "" {
		return errors.New("listener.tls.cert_path and listener.tls.key_path must both be set for https")
	}
	if cfg.TLS.ClientAuth.ClientCAPath == "" {
		return errors.New("listener.tls.client_auth.client_ca_path must be set when listener.tls.client_auth.mode is mtls")
	}
	return nil
}

func (mode ListenerTransportMode) Validate() error {
	switch mode {
	case ListenerTransportModeHTTPS:
		return nil
	default:
		return errors.New("must be https")
	}
}

func (mode ListenerClientAuthMode) Validate() error {
	switch mode {
	case ListenerClientAuthModeMTLS:
		return nil
	default:
		return errors.New("must be mtls")
	}
}
