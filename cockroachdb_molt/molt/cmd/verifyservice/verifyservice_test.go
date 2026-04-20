package verifyservice_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	rootcmd "github.com/cockroachdb/molt/cmd"
	"github.com/stretchr/testify/require"
)

func parseJSONLogLine(t *testing.T, raw string) map[string]any {
	t.Helper()
	lines := bytes.Split(bytes.TrimSpace([]byte(raw)), []byte("\n"))
	require.Len(t, lines, 1, "json logging mode must emit exactly one line")

	var payload map[string]any
	require.NoError(t, json.Unmarshal(lines[0], &payload), "json logging mode must emit valid JSON")
	return payload
}

type lockedBuffer struct {
	mu  sync.Mutex
	buf bytes.Buffer
}

func (b *lockedBuffer) Write(p []byte) (int, error) {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.Write(p)
}

func (b *lockedBuffer) String() string {
	b.mu.Lock()
	defer b.mu.Unlock()
	return b.buf.String()
}

func writeRuntimeConfig(t *testing.T) string {
	t.Helper()

	tempDir := t.TempDir()
	configPath := filepath.Join(tempDir, "verify-service.yml")
	serverCertPath := locateRepoPath(t, "crates", "runner", "tests", "fixtures", "certs", "server.crt")
	serverKeyPath := locateRepoPath(t, "crates", "runner", "tests", "fixtures", "certs", "server.key")
	clientCAPath := locateRepoPath(t, "investigations", "cockroach-webhook-cdc", "certs", "ca.crt")

	config := fmt.Sprintf(`listener:
  bind_addr: 127.0.0.1:0
  transport:
    mode: https
  tls:
    cert_path: %s
    key_path: %s
    client_auth:
      mode: mtls
      client_ca_path: %s
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb
    tls:
      mode: verify-full
      ca_cert_path: %s
      client_cert_path: %s
      client_key_path: %s
  destination:
    url: postgresql://verify_target@crdb.internal:26257/appdb
    tls:
      mode: verify-ca
      ca_cert_path: %s
`,
		serverCertPath,
		serverKeyPath,
		clientCAPath,
		clientCAPath,
		serverCertPath,
		serverKeyPath,
		clientCAPath,
	)
	require.NoError(t, os.WriteFile(configPath, []byte(config), 0o600))
	return configPath
}

func locateRepoPath(t *testing.T, parts ...string) string {
	t.Helper()

	wd, err := os.Getwd()
	require.NoError(t, err)

	candidates := []string{
		filepath.Join(append([]string{wd}, parts...)...),
		filepath.Join(append([]string{wd, ".."}, parts...)...),
		filepath.Join(append([]string{wd, "..", ".."}, parts...)...),
		filepath.Join(append([]string{wd, "..", "..", ".."}, parts...)...),
		filepath.Join(append([]string{wd, "..", "..", "..", ".."}, parts...)...),
	}

	for _, candidate := range candidates {
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
	}

	t.Fatalf("failed to locate repo path for %v from %s", parts, wd)
	return ""
}

func TestValidateConfig(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{
		"verify-service",
		"validate-config",
		"--config",
		filepath.Join("..", "..", "verifyservice", "testdata", "valid-https-mtls.yml"),
	})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())
	require.Equal(t, ""+
		"verify-service config is valid\n"+
		"listener transport: https\n"+
		"listener client auth: mtls\n"+
		"source tls mode: verify-full\n"+
		"destination tls mode: verify-ca\n",
		stdout.String(),
	)
	require.Empty(t, stderr.String())
}

func TestValidateConfigSupportsPasswordlessClientCertificates(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{
		"verify-service",
		"validate-config",
		"--config",
		filepath.Join("..", "..", "verifyservice", "testdata", "valid-passwordless-client-cert.yml"),
	})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())
	require.Equal(t, ""+
		"verify-service config is valid\n"+
		"listener transport: https\n"+
		"listener client auth: mtls\n"+
		"source tls mode: verify-full\n"+
		"destination tls mode: verify-ca\n",
		stdout.String(),
	)
	require.Empty(t, stderr.String())
}

func TestValidateConfigSupportsJSONOperatorLogs(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{
		"verify-service",
		"validate-config",
		"--log-format",
		"json",
		"--config",
		filepath.Join("..", "..", "verifyservice", "testdata", "valid-https-mtls.yml"),
	})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())
	require.Empty(t, stdout.String(), "json logging mode must not emit the legacy plain-text summary on stdout")

	payload := parseJSONLogLine(t, stderr.String())
	for _, key := range []string{"timestamp", "level", "service", "event", "message"} {
		require.Contains(t, payload, key, "verify-service json log must include %s", key)
	}
	require.Equal(t, "verify", payload["service"])
	require.Equal(t, "config.validated", payload["event"])
}

func TestValidateConfigReportsInvalidConfigAsJSONError(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{
		"verify-service",
		"validate-config",
		"--log-format",
		"json",
		"--config",
		filepath.Join("..", "..", "verifyservice", "testdata", "invalid-http-listener.yml"),
	})

	err := cmd.Execute()
	require.Error(t, err)
	require.Empty(t, stdout.String(), "invalid config must not emit a success summary on stdout")

	payload := parseJSONLogLine(t, stderr.String())
	for _, key := range []string{"timestamp", "level", "service", "event", "message"} {
		require.Contains(t, payload, key, "verify-service json error log must include %s", key)
	}
	require.Equal(t, "error", payload["level"])
	require.Equal(t, "verify", payload["service"])
	require.Equal(t, "command.failed", payload["event"])
	require.Contains(t, payload["message"].(string), "listener.transport.mode")
}

func TestRunSupportsJSONOperatorLogsForRuntimeStartup(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	cmd := rootcmd.NewRootCmd()
	stdout := new(lockedBuffer)
	stderr := new(lockedBuffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetContext(ctx)
	cmd.SetArgs([]string{
		"verify-service",
		"run",
		"--log-format",
		"json",
		"--config",
		writeRuntimeConfig(t),
	})

	errCh := make(chan error, 1)
	go func() {
		errCh <- cmd.Execute()
	}()

	var rawLogs string
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		rawLogs = stderr.String()
		if strings.Contains(rawLogs, "\n") {
			break
		}
		select {
		case err := <-errCh:
			require.NoError(t, err)
			t.Fatal("verify-service run exited before emitting the startup log")
		default:
		}
		time.Sleep(25 * time.Millisecond)
	}
	require.Contains(t, rawLogs, "\n", "verify-service run must emit a startup log line")

	payload := parseJSONLogLine(t, rawLogs)
	for _, key := range []string{"timestamp", "level", "service", "event", "message"} {
		require.Contains(t, payload, key, "verify-service runtime json log must include %s", key)
	}
	require.Equal(t, "verify", payload["service"])
	require.Equal(t, "runtime.starting", payload["event"])
	require.Empty(t, stdout.String(), "verify-service run must not emit stdout payloads in json mode")

	cancel()
	require.NoError(t, <-errCh)
}

func TestRunReportsInvalidConfigAsJSONError(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{
		"verify-service",
		"run",
		"--log-format",
		"json",
		"--config",
		filepath.Join("..", "..", "verifyservice", "testdata", "invalid-http-listener.yml"),
	})

	err := cmd.Execute()
	require.Error(t, err)
	require.Empty(t, stdout.String(), "invalid runtime config must not emit stdout output")

	payload := parseJSONLogLine(t, stderr.String())
	for _, key := range []string{"timestamp", "level", "service", "event", "message"} {
		require.Contains(t, payload, key, "verify-service runtime json error must include %s", key)
	}
	require.Equal(t, "error", payload["level"])
	require.Equal(t, "verify", payload["service"])
	require.Equal(t, "command.failed", payload["event"])
	require.Contains(t, payload["message"].(string), "listener.transport.mode")
}

func TestValidateConfigHelpStaysConfigOnly(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{"verify-service", "validate-config", "--help"})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())
	require.Contains(t, stdout.String(), "--config string")
	require.NotContains(t, stdout.String(), "--source")
	require.NotContains(t, stdout.String(), "--target")
	require.NotContains(t, stdout.String(), "--source-url")
	require.NotContains(t, stdout.String(), "--target-url")
	require.NotContains(t, stdout.String(), "verify-full override")
	require.Empty(t, stderr.String())
}

func TestRunHelpStaysConfigOnly(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{"verify-service", "run", "--help"})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())
	require.Contains(t, stdout.String(), "--config string")
	require.NotContains(t, stdout.String(), "--source")
	require.NotContains(t, stdout.String(), "--target")
	require.NotContains(t, stdout.String(), "--source-url")
	require.NotContains(t, stdout.String(), "--target-url")
	require.Empty(t, stderr.String())
}
