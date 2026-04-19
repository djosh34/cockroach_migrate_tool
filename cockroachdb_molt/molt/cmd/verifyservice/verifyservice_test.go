package verifyservice_test

import (
	"bytes"
	"path/filepath"
	"testing"

	rootcmd "github.com/cockroachdb/molt/cmd"
	"github.com/stretchr/testify/require"
)

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
