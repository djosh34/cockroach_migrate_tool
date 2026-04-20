package verifyservice_test

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net"
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

func parseJSONLogLines(t *testing.T, raw string) []map[string]any {
	t.Helper()

	lines := bytes.Split(bytes.TrimSpace([]byte(raw)), []byte("\n"))
	payloads := make([]map[string]any, 0, len(lines))
	for _, line := range lines {
		if len(bytes.TrimSpace(line)) == 0 {
			continue
		}
		var payload map[string]any
		require.NoError(t, json.Unmarshal(line, &payload), "json logging mode must emit valid JSON")
		payloads = append(payloads, payload)
	}
	require.NotEmpty(t, payloads, "json logging mode must emit at least one log line")
	return payloads
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
  tls:
    cert_path: %s
    key_path: %s
    client_ca_path: %s
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    ca_cert_path: %s
    client_cert_path: %s
    client_key_path: %s
  destination:
    url: postgresql://verify_target@crdb.internal:26257/appdb?sslmode=verify-ca
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

func writeHTTPRuntimeConfig(t *testing.T, bindAddr string) string {
	t.Helper()

	tempDir := t.TempDir()
	configPath := filepath.Join(tempDir, "verify-service.yml")
	config := fmt.Sprintf(`listener:
  bind_addr: %s
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=disable
  destination:
    url: postgresql://verify_target@crdb.internal:26257/appdb?sslmode=disable
`, bindAddr)
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
		"listener mode: https+mtls\n"+
		"source sslmode: verify-full\n"+
		"destination sslmode: verify-ca\n",
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
		"listener mode: https+mtls\n"+
		"source sslmode: verify-full\n"+
		"destination sslmode: verify-ca\n",
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
	require.Equal(t, "https+mtls", payload["listener_mode"])
	require.Equal(t, "verify-full", payload["source_sslmode"])
	require.Equal(t, "verify-ca", payload["destination_sslmode"])
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
		filepath.Join("..", "..", "verifyservice", "testdata", "invalid-https-without-server-cert.yml"),
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
	require.Equal(t, "config", payload["category"])
	require.Equal(t, "invalid_config", payload["code"])
	require.Equal(t, "verify-service config is invalid", payload["message"])
	require.Equal(
		t,
		[]any{
			map[string]any{
				"reason": "listener.tls.cert_path and listener.tls.key_path must both be set when listener.tls is configured",
			},
		},
		payload["details"],
	)
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
		filepath.Join("..", "..", "verifyservice", "testdata", "invalid-https-without-server-cert.yml"),
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
	require.Equal(t, "config", payload["category"])
	require.Equal(t, "invalid_config", payload["code"])
	require.Equal(t, "verify-service config is invalid", payload["message"])
	require.Equal(
		t,
		[]any{
			map[string]any{
				"reason": "listener.tls.cert_path and listener.tls.key_path must both be set when listener.tls is configured",
			},
		},
		payload["details"],
	)
}

func TestRunReportsStartupFailureAsJSONError(t *testing.T) {
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	require.NoError(t, err)
	defer func() {
		_ = listener.Close()
	}()

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
		writeHTTPRuntimeConfig(t, listener.Addr().String()),
	})

	err = cmd.Execute()
	require.Error(t, err)
	require.Empty(t, stdout.String(), "startup failure must not emit stdout output")

	payloads := parseJSONLogLines(t, stderr.String())
	require.GreaterOrEqual(t, len(payloads), 2, "runtime startup failure should log the start attempt and the failure")
	require.Equal(t, "runtime.starting", payloads[0]["event"])

	failure := payloads[len(payloads)-1]
	require.Equal(t, "error", failure["level"])
	require.Equal(t, "verify", failure["service"])
	require.Equal(t, "command.failed", failure["event"])
	require.Equal(t, "startup", failure["category"])
	require.Equal(t, "listener_start_failed", failure["code"])
	require.Equal(t, "verify-service listener failed to start", failure["message"])
	require.Equal(
		t,
		[]any{
			map[string]any{
				"reason": failure["details"].([]any)[0].(map[string]any)["reason"],
			},
		},
		failure["details"],
	)
	require.Contains(t, failure["details"].([]any)[0].(map[string]any)["reason"].(string), "address already in use")
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

func TestVerifyServiceHelpStaysAtOneActionLevel(t *testing.T) {
	cmd := rootcmd.NewRootCmd()
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	cmd.SetOut(stdout)
	cmd.SetErr(stderr)
	cmd.SetArgs([]string{"verify-service", "--help"})

	err := cmd.Execute()
	require.NoError(t, err, stderr.String())

	helpOutput := stdout.String()
	require.Contains(t, helpOutput, "Commands for validating and running the dedicated verify service configuration.")
	require.Contains(t, helpOutput, "run             Run the dedicated verify-service HTTP API.")
	require.Contains(t, helpOutput, "validate-config Validate the dedicated verify-service config file.")
	require.Contains(t, helpOutput, "Use \"molt verify-service [command] --help\" for more information about a command.")
	require.NotContains(t, helpOutput, "fetch")
	require.NotContains(t, helpOutput, "export")
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
