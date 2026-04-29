package verifyservice

import (
	"bytes"
	"fmt"
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
	Source         *DatabaseConfig         `yaml:"source,omitempty"`
	Destination    *DatabaseConfig         `yaml:"destination,omitempty"`
	Databases      []DatabaseMappingConfig `yaml:"databases"`
	RawTableOutput bool                    `yaml:"raw_table_output"`
}

type DatabaseConfig struct {
	Host     string             `yaml:"host,omitempty"`
	Port     int                `yaml:"port,omitempty"`
	Database string             `yaml:"database,omitempty"`
	Username CredentialValue    `yaml:"username,omitempty"`
	Password CredentialValue    `yaml:"password,omitempty"`
	SSLMode  string             `yaml:"sslmode,omitempty"`
	TLS      *DatabaseTLSConfig `yaml:"tls,omitempty"`
}

type DatabaseMappingConfig struct {
	Name                string          `yaml:"name"`
	SourceDatabase      string          `yaml:"source_database,omitempty"`
	DestinationDatabase string          `yaml:"destination_database,omitempty"`
	Source              *DatabaseConfig `yaml:"source,omitempty"`
	Destination         *DatabaseConfig `yaml:"destination,omitempty"`
}

type DatabaseTLSConfig struct {
	CACertPath     string `yaml:"ca_cert_path,omitempty"`
	ClientCertPath string `yaml:"client_cert_path,omitempty"`
	ClientKeyPath  string `yaml:"client_key_path,omitempty"`
}

type CredentialValue struct {
	Value      string `yaml:"value,omitempty"`
	EnvRef     string `yaml:"env_ref,omitempty"`
	SecretFile string `yaml:"secret_file,omitempty"`

	present       bool
	valueSet      bool
	envRefSet     bool
	secretFileSet bool
}

type verifyConfigDecode struct {
	Source         *DatabaseConfig `yaml:"source,omitempty"`
	Destination    *DatabaseConfig `yaml:"destination,omitempty"`
	Databases      []yaml.Node     `yaml:"databases"`
	RawTableOutput bool            `yaml:"raw_table_output"`
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
		return Config{}, asInvalidConfigError(err)
	}
	return cfg, nil
}

func (cfg Config) Validate() error {
	if err := cfg.Listener.validate(); err != nil {
		return err
	}
	if err := cfg.Verify.validate(); err != nil {
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

func (cfg VerifyConfig) validate() error {
	if len(cfg.Databases) == 0 {
		return errors.New("verify.databases must contain at least one database mapping")
	}

	seenNames := make(map[string]struct{}, len(cfg.Databases))
	for index, database := range cfg.Databases {
		path := fmt.Sprintf("verify.databases[%d]", index)
		if database.Name == "" {
			return errors.Newf("%s.name must be set", path)
		}
		if _, exists := seenNames[database.Name]; exists {
			return errors.Newf("%s.name duplicates configured database %q", path, database.Name)
		}
		seenNames[database.Name] = struct{}{}

		if _, err := cfg.resolveDatabaseAt(path, database); err != nil {
			return err
		}
	}
	return nil
}

func (cfg *VerifyConfig) UnmarshalYAML(node *yaml.Node) error {
	var decoded verifyConfigDecode
	if err := decodeKnownFieldsNode(node, &decoded); err != nil {
		return err
	}

	databases := make([]DatabaseMappingConfig, 0, len(decoded.Databases))
	for index, databaseNode := range decoded.Databases {
		if databaseNode.Kind != yaml.MappingNode {
			return errors.Newf("verify.databases[%d] must be a mapping object", index)
		}

		var database DatabaseMappingConfig
		if err := decodeKnownFieldsNode(&databaseNode, &database); err != nil {
			return err
		}
		databases = append(databases, database)
	}

	*cfg = VerifyConfig{
		Source:         decoded.Source,
		Destination:    decoded.Destination,
		Databases:      databases,
		RawTableOutput: decoded.RawTableOutput,
	}
	return nil
}

func decodeKnownFieldsNode(node *yaml.Node, target any) error {
	content, err := yaml.Marshal(node)
	if err != nil {
		return err
	}

	decoder := yaml.NewDecoder(bytes.NewReader(content))
	decoder.KnownFields(true)
	return decoder.Decode(target)
}

func (value *CredentialValue) UnmarshalYAML(node *yaml.Node) error {
	if value == nil {
		return errors.New("credential target must not be nil")
	}

	switch node.Kind {
	case yaml.ScalarNode:
		var literal string
		if err := node.Decode(&literal); err != nil {
			return err
		}
		*value = CredentialValue{
			Value:    literal,
			present:  true,
			valueSet: true,
		}
		return nil
	case yaml.MappingNode:
		var decoded struct {
			Value      *string `yaml:"value,omitempty"`
			EnvRef     *string `yaml:"env_ref,omitempty"`
			SecretFile *string `yaml:"secret_file,omitempty"`
		}
		if err := decodeKnownFieldsNode(node, &decoded); err != nil {
			return err
		}

		next := CredentialValue{present: true}
		if decoded.Value != nil {
			next.Value = *decoded.Value
			next.valueSet = true
		}
		if decoded.EnvRef != nil {
			next.EnvRef = *decoded.EnvRef
			next.envRefSet = true
		}
		if decoded.SecretFile != nil {
			next.SecretFile = *decoded.SecretFile
			next.secretFileSet = true
		}
		*value = next
		return nil
	default:
		return errors.New("credential value must be a string or mapping object")
	}
}

func (value CredentialValue) declared() bool {
	return value.present || value.valueSet || value.envRefSet || value.secretFileSet ||
		value.Value != "" || value.EnvRef != "" || value.SecretFile != ""
}

func (value CredentialValue) sourceCount() int {
	count := 0
	if value.valueSet || value.Value != "" {
		count++
	}
	if value.envRefSet || value.EnvRef != "" {
		count++
	}
	if value.secretFileSet || value.SecretFile != "" {
		count++
	}
	return count
}

func asInvalidConfigError(err error) error {
	if err == nil {
		return nil
	}
	if opErr, ok := asOperatorError(err); ok && opErr.category == "config" && opErr.code == "invalid_config" {
		return opErr
	}
	return newOperatorError(
		"config",
		"invalid_config",
		"verify-service config is invalid",
		operatorErrorDetail{Reason: err.Error()},
	)
}
