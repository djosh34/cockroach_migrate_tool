package verifyservice_test

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/cockroachdb/molt/verifyservice"
	dto "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"
	"github.com/stretchr/testify/require"
)

func TestPostJobsStartsJobForAllConfiguredDatabases(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(newVerifyServiceConfig("app", "billing"), verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	job := postJob(t, server.URL, `{}`)
	require.Equal(t, "job-000001", job.JobID)
	require.Equal(t, "running", job.Status)
	require.Nil(t, job.FinishedAt)
	require.Len(t, job.Databases, 2)
	require.Equal(t, []string{"app", "billing"}, databaseNames(job.Databases))
	for _, database := range job.Databases {
		require.Equal(t, "running", database.Status)
		require.NotEmpty(t, database.StartedAt)
		require.Nil(t, database.FinishedAt)
		require.Nil(t, database.Schemas)
		require.Nil(t, database.Tables)
		require.Nil(t, database.RowsChecked)
		require.Nil(t, database.Error)
		require.Nil(t, database.Findings)
	}

	select {
	case request := <-runner.started:
		require.Equal(t, utils.DefaultFilterConfig(), request.FilterConfig())
	case <-time.After(2 * time.Second):
		t.Fatal("expected verify job to start")
	}
}

func TestGetJobsAndGetJobShareCanonicalSchema(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 2)}
	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001", "job-000002"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	firstJob := postJob(t, server.URL, `{}`)
	secondJob := postJob(t, server.URL, `{}`)
	require.Equal(t, "job-000001", firstJob.JobID)
	require.Equal(t, "job-000002", secondJob.JobID)

	listedJobs := listJobs(t, server.URL)
	require.Len(t, listedJobs, 2)

	firstListedJob := listedJobs[0]
	secondListedJob := listedJobs[1]
	require.Equal(t, firstJob, firstListedJob)
	require.Equal(t, secondJob, secondListedJob)
	require.Equal(t, firstListedJob, getJob(t, server.URL, "job-000001"))
	require.Equal(t, secondListedJob, getJob(t, server.URL, "job-000002"))
}

func TestPostJobsRejectsLegacyRegexFilterFields(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{"include_schema":"^public$"}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)
	require.Equal(t, operatorErrorResponse{
		Error: operatorErrorPayload{
			Category: "request_validation",
			Code:     "unknown_field",
			Message:  "request body contains an unsupported field",
			Details: []operatorErrorDetail{
				{
					Field:  "include_schema",
					Reason: "unknown field",
				},
			},
		},
	}, decodeOperatorErrorResponse(t, response))
}

func TestPostJobsRejectsInvalidGlob(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{"default_schema_match":"["}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)
	require.Equal(t, operatorErrorResponse{
		Error: operatorErrorPayload{
			Category: "request_validation",
			Code:     "invalid_glob",
			Message:  "request validation failed",
			Details: []operatorErrorDetail{
				{
					Field:  "default_schema_match",
					Reason: "syntax error in pattern",
				},
			},
		},
	}, decodeOperatorErrorResponse(t, response))
}

func TestPostJobsRejectsConnectionLikeRequestFields(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{"verify":{"source":"postgresql://secret@source/app"}}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)
	require.Equal(t, operatorErrorResponse{
		Error: operatorErrorPayload{
			Category: "request_validation",
			Code:     "unknown_field",
			Message:  "request body contains an unsupported field",
			Details: []operatorErrorDetail{
				{
					Field:  "verify",
					Reason: "unknown field",
				},
			},
		},
	}, decodeOperatorErrorResponse(t, response))
}

func TestPostJobStopReturnsCanonicalJobObject(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	postJob(t, server.URL, `{}`)

	stopResponse, err := http.Post(server.URL+"/jobs/job-000001/stop", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = stopResponse.Body.Close()
	})
	require.Equal(t, http.StatusOK, stopResponse.StatusCode)

	var stoppingJob jobResponse
	require.NoError(t, json.NewDecoder(stopResponse.Body).Decode(&stoppingJob))
	require.Equal(t, "stopping", stoppingJob.Status)
	require.Len(t, stoppingJob.Databases, 1)
	require.Equal(t, "stopping", stoppingJob.Databases[0].Status)

	require.Eventually(t, func() bool {
		job := getJob(t, server.URL, "job-000001")
		return job.Status == "stopped" && job.Databases[0].Status == "stopped" && job.FinishedAt != nil
	}, 2*time.Second, 20*time.Millisecond)
}

func TestDatabaseFailuresStayScopedToDatabaseEntries(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.SummaryReport{
			Stats: inconsistency.RowStats{
				Schema:            "public",
				Table:             "accounts",
				NumVerified:       7,
				NumSuccess:        6,
				NumColumnMismatch: 1,
			},
		})
		reporter.Report(inconsistency.MismatchingColumn{
			Name: dbtable.Name{
				Schema: "public",
				Table:  "accounts",
			},
			PrimaryKeyColumns: []tree.Name{"id"},
			PrimaryKeyValues:  tree.Datums{tree.NewDInt(101)},
			MismatchingColumns: []tree.Name{
				"balance",
			},
			TruthVals:  tree.Datums{tree.NewDInt(17)},
			TargetVals: tree.Datums{tree.NewDInt(23)},
			Info:       []string{"balance mismatch"},
		})
		return nil
	})
	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	postJob(t, server.URL, `{}`)

	require.Eventually(t, func() bool {
		job := getJob(t, server.URL, "job-000001")
		if job.Status != "failed" {
			return false
		}

		require.Len(t, job.Databases, 1)
		database := job.Databases[0]
		require.Equal(t, "app", database.Name)
		require.Equal(t, "failed", database.Status)
		require.NotNil(t, database.Error)
		require.Equal(t, "mismatch", database.Error.Category)
		require.Equal(t, "mismatch_detected", database.Error.Code)
		require.NotNil(t, database.RowsChecked)
		require.Equal(t, 7, *database.RowsChecked)
		require.Equal(t, []string{"public"}, database.Schemas)
		require.Equal(t, []string{"accounts"}, database.Tables)
		require.Len(t, database.Findings, 1)

		rawJob := getJobMap(t, server.URL, "job-000001")
		_, hasTopLevelError := rawJob["error"]
		_, hasTopLevelFailure := rawJob["failure"]
		_, hasTopLevelResult := rawJob["result"]
		require.False(t, hasTopLevelError)
		require.False(t, hasTopLevelFailure)
		require.False(t, hasTopLevelResult)
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestGetJobsReturnsRetainedCompletedJobs(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		IDGenerator: sequentialIDGenerator("job-000001", "job-000002"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	postJob(t, server.URL, `{}`)
	postJob(t, server.URL, `{}`)

	require.Eventually(t, func() bool {
		jobs := listJobs(t, server.URL)
		if len(jobs) != 2 {
			return false
		}
		return jobs[0].Status == "succeeded" && jobs[1].Status == "succeeded"
	}, 2*time.Second, 20*time.Millisecond)
}

func TestMetricsReportConcurrentRunningJobs(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 2)}
	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001", "job-000002"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	postJob(t, server.URL, `{}`)
	postJob(t, server.URL, `{}`)

	response, err := http.Get(server.URL + "/metrics")
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})
	require.Equal(t, http.StatusOK, response.StatusCode)

	body, err := io.ReadAll(response.Body)
	require.NoError(t, err)
	metrics := string(body)
	require.Contains(t, metrics, "cockroach_migration_tool_verify_active_jobs 2")
	require.Contains(t, metrics, `cockroach_migration_tool_verify_jobs_total{status="running"} 2`)
}

func TestMetricsExposeLifecycleLabelsOnly(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error {
			return errors.New("verify exploded with free text")
		}),
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	postJob(t, server.URL, `{}`)

	require.Eventually(t, func() bool {
		response, err := http.Get(server.URL + "/metrics")
		require.NoError(t, err)
		defer func() {
			_ = response.Body.Close()
		}()
		if response.StatusCode != http.StatusOK {
			return false
		}

		body, err := io.ReadAll(response.Body)
		require.NoError(t, err)
		metrics := string(body)
		if !strings.Contains(metrics, `cockroach_migration_tool_verify_jobs_total{status="failed"} 1`) {
			return false
		}

		parser := expfmt.TextParser{}
		families, err := parser.TextToMetricFamilies(strings.NewReader(metrics))
		require.NoError(t, err)
		require.Empty(t, metricLabelNames(t, families["cockroach_migration_tool_verify_active_jobs"]))
		require.Equal(t, []string{"status"}, metricLabelNames(t, families["cockroach_migration_tool_verify_jobs_total"]))
		require.NotContains(t, metrics, "verify exploded with free text")
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestJobResultsAreLostAfterProcessRestart(t *testing.T) {
	t.Parallel()

	serviceOne := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner:      reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(serviceOne.Close)

	serverOne := httptest.NewServer(serviceOne.Handler())
	t.Cleanup(serverOne.Close)

	postJob(t, serverOne.URL, `{}`)

	require.Eventually(t, func() bool {
		return getJob(t, serverOne.URL, "job-000001").Status == "succeeded"
	}, 2*time.Second, 20*time.Millisecond)

	serverOne.Close()
	serviceOne.Close()

	serviceTwo := verifyservice.NewService(newVerifyServiceConfig("app"), verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(serviceTwo.Close)

	serverTwo := httptest.NewServer(serviceTwo.Handler())
	t.Cleanup(serverTwo.Close)

	response, err := http.Get(serverTwo.URL + "/jobs/job-000001")
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})
	require.Equal(t, http.StatusNotFound, response.StatusCode)
}

type blockingRunner struct {
	started chan verifyservice.RunRequest
}

func (r *blockingRunner) Run(
	ctx context.Context,
	request verifyservice.RunRequest,
	_ inconsistency.Reporter,
) error {
	r.started <- request
	<-ctx.Done()
	return ctx.Err()
}

type reportingRunner func(ctx context.Context, reporter inconsistency.Reporter) error

func (r reportingRunner) Run(
	ctx context.Context,
	_ verifyservice.RunRequest,
	reporter inconsistency.Reporter,
) error {
	return r(ctx, reporter)
}

func sequentialIDGenerator(ids ...string) func() string {
	return func() string {
		if len(ids) == 0 {
			return "unexpected-id"
		}
		next := ids[0]
		ids = ids[1:]
		return next
	}
}

type operatorErrorResponse struct {
	Error operatorErrorPayload `json:"error"`
}

type operatorErrorPayload struct {
	Category string                `json:"category"`
	Code     string                `json:"code"`
	Message  string                `json:"message"`
	Details  []operatorErrorDetail `json:"details,omitempty"`
}

type operatorErrorDetail struct {
	Field  string `json:"field,omitempty"`
	Reason string `json:"reason,omitempty"`
}

type jobResponse struct {
	JobID      string             `json:"job_id"`
	Status     string             `json:"status"`
	CreatedAt  string             `json:"created_at"`
	StartedAt  string             `json:"started_at"`
	FinishedAt *string            `json:"finished_at"`
	Databases  []databaseResponse `json:"databases"`
}

type databaseResponse struct {
	Name        string                `json:"name"`
	Status      string                `json:"status"`
	StartedAt   string                `json:"started_at"`
	FinishedAt  *string               `json:"finished_at"`
	Schemas     []string              `json:"schemas"`
	Tables      []string              `json:"tables"`
	RowsChecked *int                  `json:"rows_checked"`
	Error       *operatorErrorPayload `json:"error"`
	Findings    []map[string]any      `json:"findings"`
}

func postJob(t *testing.T, serverURL string, body string) jobResponse {
	t.Helper()

	response, err := http.Post(serverURL+"/jobs", "application/json", bytes.NewBufferString(body))
	require.NoError(t, err)
	defer func() {
		_ = response.Body.Close()
	}()
	require.Equal(t, http.StatusAccepted, response.StatusCode)

	var job jobResponse
	require.NoError(t, json.NewDecoder(response.Body).Decode(&job))
	require.NotEmpty(t, job.CreatedAt)
	require.NotEmpty(t, job.StartedAt)
	return job
}

func getJob(t *testing.T, serverURL string, jobID string) jobResponse {
	t.Helper()

	response, err := http.Get(serverURL + "/jobs/" + jobID)
	require.NoError(t, err)
	defer func() {
		_ = response.Body.Close()
	}()
	require.Equal(t, http.StatusOK, response.StatusCode)

	var job jobResponse
	require.NoError(t, json.NewDecoder(response.Body).Decode(&job))
	return job
}

func getJobMap(t *testing.T, serverURL string, jobID string) map[string]any {
	t.Helper()

	response, err := http.Get(serverURL + "/jobs/" + jobID)
	require.NoError(t, err)
	defer func() {
		_ = response.Body.Close()
	}()
	require.Equal(t, http.StatusOK, response.StatusCode)

	var payload map[string]any
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	return payload
}

func listJobs(t *testing.T, serverURL string) []jobResponse {
	t.Helper()

	response, err := http.Get(serverURL + "/jobs")
	require.NoError(t, err)
	defer func() {
		_ = response.Body.Close()
	}()
	require.Equal(t, http.StatusOK, response.StatusCode)

	var jobs []jobResponse
	require.NoError(t, json.NewDecoder(response.Body).Decode(&jobs))
	return jobs
}

func newVerifyServiceConfig(databaseNames ...string) verifyservice.Config {
	databases := make([]verifyservice.DatabaseMappingConfig, 0, len(databaseNames))
	for _, databaseName := range databaseNames {
		databases = append(databases, verifyservice.DatabaseMappingConfig{
			Name:                databaseName,
			SourceDatabase:      databaseName,
			DestinationDatabase: databaseName,
		})
	}

	return verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			Source: &verifyservice.DatabaseConfig{
				Host:    "source.internal",
				Port:    26257,
				User:    "verify_source",
				SSLMode: "disable",
			},
			Destination: &verifyservice.DatabaseConfig{
				Host:    "destination.internal",
				Port:    5432,
				User:    "verify_target",
				SSLMode: "disable",
			},
			Databases: databases,
		},
	}
}

func decodeOperatorErrorResponse(t *testing.T, response *http.Response) operatorErrorResponse {
	t.Helper()

	var payload operatorErrorResponse
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	return payload
}

func databaseNames(databases []databaseResponse) []string {
	names := make([]string, 0, len(databases))
	for _, database := range databases {
		names = append(names, database.Name)
	}
	return names
}

func metricLabelNames(t *testing.T, family *dto.MetricFamily) []string {
	t.Helper()

	require.NotNil(t, family)
	require.NotEmpty(t, family.Metric)

	names := make([]string, 0, len(family.Metric[0].Label))
	for _, label := range family.Metric[0].Label {
		names = append(names, label.GetName())
	}
	sort.Strings(names)
	return names
}
