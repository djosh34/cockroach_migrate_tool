package e2e

import (
	"fmt"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"

	"github.com/cockroachdb/cockroach-go/v2/testserver"
	"github.com/cockroachdb/datadriven"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/testutils"
	"github.com/stretchr/testify/require"
)

func TestDataDriven(t *testing.T) {
	datadriven.Walk(t, "testdata", func(t *testing.T, path string) {
		driverDialect := filepath.Base(filepath.Dir(path))
		// So that we can have sub-directories.
		switch driverDialect {
		case "pg", "mysql", "crdb", "crdb-multi-region":
		default:
			return
		}

		var srcTs, tgtTs testserver.TestServer

		if driverDialect == "crdb-multi-region" {
			srcTs, tgtTs = SetupCRDBMRTestServers(t)
		} else {
			require.NoError(t, Setup(driverDialect))
			t.Log("finished setup")
			require.NoError(t, ConfirmContainersRunning(t, driverDialect))
			t.Log("containers are all up")
		}

		defer func() {
			// --no-clean-containers
			if !*flagNoCleanContainers {
				t.Log("tearing down containers")
				require.NoError(t, TearDown())
				t.Log("all containers are terminated")

				if srcTs != nil {
					srcTs.Stop()
					t.Log("source test server is stopped")
				}
				if tgtTs != nil {
					tgtTs.Stop()
					t.Log("target test server is stopped")
				}
			}
		}()

		datadriven.RunTestAny(t, path, func(t testing.TB, d *datadriven.TestData) string {
			// Remove common args.
			var silent bool
			newArgs := d.CmdArgs[:0]

			for _, arg := range d.CmdArgs {
				switch arg.Key {
				case "silent":
					silent = true
					continue
				}
				newArgs = append(newArgs, arg)
			}
			d.CmdArgs = newArgs
			var expectError bool

			switch d.Cmd {
			case "exec":
				var stdout strings.Builder
				var stderr strings.Builder

				for _, arg := range d.CmdArgs {
					switch arg.Key {
					case "expect-error":
						expectError = true
					default:
					}
				}
				cmd := exec.Command("/bin/sh", "-c", d.Input)
				cmd.Stdout = &stdout
				cmd.Stderr = &stderr
				err := cmd.Run()
				if err != nil {
					errMessage := stderr.String()
					if expectError {
						return strings.TrimSpace(errMessage)
					}
					t.Fatalf(errors.Wrapf(errors.New(errMessage), "error executing %s", strings.Join(cmd.Args, " ")).Error())
				}
				if silent {
					return ""
				}
				return strings.TrimSpace(stdout.String())
			case "fetch", "verify":
				if len(d.CmdArgs) < 3 {
					t.Fatalf("expect at least 2 args for %q command", d.Cmd)
				}

				toRunCmd := fmt.Sprintf(`go run .. %s --test-only %s`, d.Cmd, testutils.GetCmdArgsStr(d.CmdArgs))
				t.Logf("running %q", toRunCmd)
				cmd := exec.Command("/bin/sh", "-c", toRunCmd)

				var stdout strings.Builder
				var stderr strings.Builder
				cmd.Stdout = &stdout
				cmd.Stderr = &stderr
				err := cmd.Run()
				if err != nil {
					errStr := stderr.String()
					t.Logf(errors.Wrapf(errors.New(errStr), "error executing molt %s", d.Cmd).Error())
					return strings.TrimSpace(redactLogs(errStr))
				}
				return strings.TrimSpace(redactLogs(stdout.String()))
			}
			t.Fatalf("unknown command: %s", d.Cmd)
			return ""
		})
	})
}

// redactLogs is to remove fields that are not deterministic.
func redactLogs(s string) string {
	// Remove the "time:xxxxx" filed of the Info logs.
	const timePattern = `\"time\":\"[0-9T\-\:TZ]*\",`
	res := regexp.MustCompile(timePattern).ReplaceAllString(s, "")

	// Remove the `starting file server` log as it is logged from a goroutine
	// whose occurrence order is not deterministic with the main log flow.

	// (?m) is added at the beginning of the pattern to enable multi-line mode.
	// This allows ^ (and $) to match the start and end of each line, not just
	// the entire string.
	const fileServerStartingPattern = `(?m)^[^\n]*starting file server[^\n]*\n?`
	return regexp.MustCompile(fileServerStartingPattern).ReplaceAllString(res, "")
}
