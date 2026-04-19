package scaletesting

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/util/uuid"
	"github.com/stretchr/testify/require"
	"golang.org/x/sync/errgroup"
)

type connRefusedError struct{}

func (m *connRefusedError) Error() string {
	return "failed to connect"
}

type awsDetails struct {
	region          string
	accessKeyID     string
	secretAccessKey string
}

var clusterName = flag.String("cluster", "", "Name of cluster")
var source = flag.String("source", "", "Source connection string")
var target = flag.String("target", "postgres://root@localhost:26257/defaultdb?sslmode=disable", "Target connection string")
var moltFetchArgs = flag.String("fetch-args", "--non-interactive --bucket-path 's3://migrations-fetch-ci-test/scale-test' --table-handling 'drop-on-target-and-recreate' --table-filter 'test_table_large' --export-concurrency 8", "Arguments for fetch")
var moltVerifyArgs = flag.String("verify-args", "", "Arguments for verify")
var roachprodCmd = flag.String("roachprod-cmd", "roachprod", "Name of the Roachprod command to run")

const pollTime = 30 * time.Second
const numLogsPerPoll = 5

func TestScale(t *testing.T) {
	ctx := context.Background()
	// Populate from flags or env later.
	clusterName := clusterName
	sourceConn := source
	targetConn := target
	fetchArgs := moltFetchArgs
	awsDetails := awsDetails{
		region:          "us-east-1",
		accessKeyID:     os.Getenv("AWS_ACCESS_KEY_ID"),
		secretAccessKey: os.Getenv("AWS_SECRET_ACCESS_KEY"),
	}

	// Construct MOLT fetch string from the inputs/flags/env
	fetchCmd := buildFetchCommand(*sourceConn, *targetConn, *fetchArgs)

	// Generate a newUUID so that the file is unique.
	newUUID, err := uuid.NewV4()
	require.NoError(t, err)

	fetchLogFileName := fmt.Sprintf("molt-fetch-%s.log", newUUID.String())

	// Run MOLT fetch on the Roachprod cluster
	_, err = runFetch(*clusterName, fetchCmd, fetchLogFileName, awsDetails)
	require.NoError(t, err)
	waitForMOLTCompletion(ctx, t, fetchLogFileName)

	// Check fetch stats which should be at the end.
	fmt.Println("final fetch logs")
	output, _ := getLogFileLines(*clusterName, fetchLogFileName, numLogsPerPoll)
	fmt.Println(output)

	// Check status of fetch once it's done (check if fetch running returned true at least once and now is not)
	fetchProcessStatus, err := getProcessStatus(*clusterName, fetchLogFileName)
	require.NoError(t, err)

	// Ensure that we end up suceeding.
	require.True(t, didProcessPass(fetchProcessStatus))

	// Run verify
	verifyLogFileName := fmt.Sprintf("molt-verify-%s.log", newUUID.String())
	verifyCmd := buildVerifyCommand(*sourceConn, *targetConn, *moltVerifyArgs)

	// Run MOLT verify on the Roachprod cluster
	_, err = runVerify(*clusterName, verifyCmd, verifyLogFileName)
	require.NoError(t, err)
	waitForMOLTCompletion(ctx, t, verifyLogFileName)

	// Check verify stats which should be at the end.
	fmt.Println("final verify logs")
	verifyOutput, _ := getLogFileLines(*clusterName, verifyLogFileName, numLogsPerPoll)
	fmt.Println(verifyOutput)

	// Check status of verify once it's done.
	verifyProcessStatus, err := getProcessStatus(*clusterName, verifyLogFileName)
	require.NoError(t, err)

	// Ensure that we end up suceeding.
	require.True(t, didProcessPass(verifyProcessStatus))

	// Check that verify log lines also note that everything passed.
	verifyLogs, err := getVerifyFinishedLogs(*clusterName, verifyLogFileName)
	require.NoError(t, err)
	fmt.Println(verifyLogs)

	verifyResults, err := extractVerifyResults(verifyLogs)
	require.NoError(t, err)

	require.True(t, didVerifyPass(verifyResults))
}

func buildFetchCommand(sourceConn, targetConn, args string) string {
	return fmt.Sprintf("./molt fetch --source '%s' --target '%s' %s", sourceConn, targetConn, args)
}

func buildVerifyCommand(sourceConn, targetConn, args string) string {
	return fmt.Sprintf("./molt verify --source '%s' --target '%s' %s", sourceConn, targetConn, args)
}

func runFetch(
	cluster, fetchCommand, logFile string, cloudDetails awsDetails,
) (pid string, err error) {
	// The semi-colon after each statement in the bracket is crucial for syntax reasons.
	command := fmt.Sprintf("export AWS_REGION='%s'; export AWS_SECRET_ACCESS_KEY='%s'; export AWS_ACCESS_KEY_ID='%s'; { %s; echo $?; } &> %s &",
		cloudDetails.region,
		cloudDetails.secretAccessKey,
		cloudDetails.accessKeyID,
		fetchCommand,
		logFile)

	cmdStruct := exec.Command(*roachprodCmd, "run", fmt.Sprintf("%s:1", cluster), "--", command)
	fmt.Println(cmdStruct.String())
	output, err := cmdStruct.CombinedOutput()
	if err != nil {
		return "", err
	}

	return string(output), nil
}

func runVerify(cluster, verifyCommand, logFile string) (pid string, err error) {
	// The semi-colon after each statement in the bracket is crucial for syntax reasons.
	command := fmt.Sprintf("{ %s; echo $?; } &> %s &",
		verifyCommand,
		logFile)

	cmdStruct := exec.Command(*roachprodCmd, "run", fmt.Sprintf("%s:1", cluster), "--", command)
	fmt.Println(cmdStruct.String())
	output, err := cmdStruct.CombinedOutput()
	if err != nil {
		return "", err
	}

	return string(output), nil
}

func waitForMOLTCompletion(ctx context.Context, t *testing.T, logFile string) {
	wg, _ := errgroup.WithContext(ctx)
	wg.Go(func() error {
		defer func() {
			// Fetch the log file to local.
			fmt.Println(logFile)
			require.NoError(t, downloadLogFile(*clusterName, logFile, fmt.Sprintf("/tmp/%s", logFile)))
		}()

		wasMOLTRunning := false
		// Poll to see if fetch is running.
		for {
			fmt.Println("checking if MOLT is still running")
			// We don't expect errors, but maybe the SSH and CURL fails.
			// TODO (rluu): we may have to make this more resilient in the future to transient network
			// conditions. After which time, we probably want to look at PIDs. Flag this in review
			// on how we can actually be more accurate here.
			// The complication in this problem is that for any type of issue running roachprod run,
			// we only get a CombinedOutput entry of "exit code 20", which is unhelpful.

			// Potential solution: if we see following error, mark it as MOLT not running error, which is an exceptional error (MOLT is done):
			// curl: (7) Failed to connect to localhost port 3030 after 0 ms: Connection refused

			// Simulated network error and noticed a few things:
			// 1. Roachprod retries on SSH problem, 3 times (which should address any transient failures)
			// 2. When retries resolve, the exit code is 10
			// 3. When an error with the curl happens, it is exit code 20
			// Can potentially just look for exit code 20 as a failure because service is no longer up => MOLT done.
			if isMOLTRunning, err := checkIfMOLTRunning(*clusterName); err != nil && !errors.Is(err, &connRefusedError{}) {
				fmt.Printf("unknown error, still looping: %s \n", err)
			} else if wasMOLTRunning && !isMOLTRunning {
				return nil
			} else if !wasMOLTRunning && isMOLTRunning {
				// Update for next iteration of the loop
				wasMOLTRunning = true
			} else if !wasMOLTRunning && !isMOLTRunning {
				return errors.New("MOLT was not running before and is still not running; probably failed to start")
			}

			// Get some observability into what's going on in the system.
			output, _ := getLogFileLines(*clusterName, logFile, numLogsPerPoll)
			fmt.Println(output)
			time.Sleep(pollTime)
		}
	})
	require.NoError(t, wg.Wait())
}

const connRefusedMarker = "Failed to connect to localhost"

func checkIfMOLTRunning(cluster string) (bool, error) {
	cmdStruct := exec.Command(*roachprodCmd, "run", fmt.Sprintf("%s:1", cluster), "--", "curl localhost:3030/healthz")
	output, err := cmdStruct.CombinedOutput()
	if strings.Contains(string(output), connRefusedMarker) {
		return false, &connRefusedError{}
	} else if err != nil {
		return false, err
	}

	result := false
	if cmdStruct.ProcessState.ExitCode() == 0 {
		result = true
	} else {
		fmt.Println("MOLT is not currently running")
	}

	return result, nil
}

func downloadLogFile(cluster, inputFileName string, destinationFileName string) error {
	cmdStruct := exec.Command(*roachprodCmd, "get", fmt.Sprintf("%s:1", cluster), "--", inputFileName, destinationFileName)
	output, err := cmdStruct.CombinedOutput()
	fmt.Println(string(output))
	if err != nil {
		return err
	}

	return nil
}

func getVerifyFinishedLogs(cluster, fileName string) (string, error) {
	commandStr := fmt.Sprintf("cat %s | grep 'finished row verification'", fileName)
	cmdStruct := exec.Command(*roachprodCmd, "run", fmt.Sprintf("%s:1", cluster), "--", commandStr)
	output, err := cmdStruct.CombinedOutput()
	if err != nil {
		return "", err
	}

	return string(output), nil
}

func getLogFileLines(cluster, fileName string, numLines int) (string, error) {
	commandStr := fmt.Sprintf("tail -n %d %s", numLines, fileName)
	cmdStruct := exec.Command(*roachprodCmd, "run", fmt.Sprintf("%s:1", cluster), "--", commandStr)
	output, err := cmdStruct.CombinedOutput()
	if err != nil {
		return "", err
	}

	return string(output), nil
}

func getProcessStatus(cluster, fileName string) (exitCode int, err error) {
	output, err := getLogFileLines(cluster, fileName, 1)
	if err != nil {
		return -1, err
	}

	outputStr := strings.TrimSpace(string(output))
	status, err := strconv.Atoi(outputStr)
	if err != nil {
		return -1, err
	}

	return status, nil
}

func didProcessPass(exitCode int) bool {
	return exitCode == 0
}

func didVerifyPass(results []*verifyResult) bool {
	for _, result := range results {
		if result.numSuccessRows != result.numTruthRows {
			fmt.Printf("error while verifying this log line: %s\n", result.originalLog)
			return false
		}
	}

	return true
}

const verifyDataPattern = `\"num_truth_rows\":(\d+).*\"num_success\":(\d+)`

var verifyDataRegEx = regexp.MustCompile(verifyDataPattern)

type verifyResult struct {
	numTruthRows   int
	numSuccessRows int
	originalLog    string
}

func extractVerifyResults(logText string) ([]*verifyResult, error) {
	lines := strings.Split(logText, "\n")
	results := []*verifyResult{}

	for _, line := range lines {
		match := verifyDataRegEx.FindStringSubmatch(line)

		if len(match) > 0 {
			truthRows, err := strconv.Atoi(match[1])
			if err != nil {
				return nil, err
			}

			successRows, err := strconv.Atoi(match[2])
			if err != nil {
				return nil, err
			}

			results = append(results, &verifyResult{
				numTruthRows:   truthRows,
				numSuccessRows: successRows,
				originalLog:    line,
			})
		}
	}

	return results, nil
}
