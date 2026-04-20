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

	body, err := io.ReadAll(getResponse.Body)
	require.NoError(t, err)

	var payload map[string]any
	require.NoError(t, json.Unmarshal(body, &payload))
	require.Equal(t, map[string]any{
		"job_id": "job-000001",
		"status": "running",
	}, payload)
}

func TestGetJobReturnsStructuredResultsAfterCompletion(t *testing.T) {
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

		body, err := io.ReadAll(getResponse.Body)
		require.NoError(t, err)

		var payload map[string]any
		require.NoError(t, json.Unmarshal(body, &payload))
		if payload["status"] != "succeeded" {
			return false
		}

		require.Equal(t, map[string]any{
			"job_id": "job-000001",
			"status": "succeeded",
			"result": map[string]any{
				"table_summaries": []any{
					map[string]any{
						"schema":              "public",
						"table":               "accounts",
						"num_verified":        float64(7),
						"num_success":         float64(0),
						"num_missing":         float64(0),
						"num_mismatch":        float64(1),
						"num_column_mismatch": float64(0),
						"num_extraneous":      float64(0),
						"num_live_retry":      float64(0),
					},
				},
				"mismatch_tables": []any{
					map[string]any{
						"schema": "public",
						"table":  "accounts",
					},
				},
				"table_definition_mismatches": []any{},
			},
		}, payload)
		require.NotContains(t, string(body), "verification in progress")
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestGetJobReturnsTableDefinitionMismatchDetailsAfterCompletion(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(_ context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary",
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 7,
				NumSuccess:  7,
			},
		})
		reporter.Report(inconsistency.MismatchingTableDefinition{
			DBTable: dbtable.DBTable{
				Name: dbtable.Name{
					Schema: "public",
					Table:  "orders",
				},
			},
			Info: "primary key mismatch",
		})
		return nil
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
		getResponse, err := http.Get(server.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = getResponse.Body.Close()
		}()
		if getResponse.StatusCode != http.StatusOK {
			return false
		}

		var payload map[string]any
		require.NoError(t, json.NewDecoder(getResponse.Body).Decode(&payload))
		if payload["status"] != "succeeded" {
			return false
		}

		require.Equal(t, map[string]any{
			"job_id": "job-000001",
			"status": "succeeded",
			"result": map[string]any{
				"table_summaries": []any{
					map[string]any{
						"schema":              "public",
						"table":               "accounts",
						"num_verified":        float64(7),
						"num_success":         float64(7),
						"num_missing":         float64(0),
						"num_mismatch":        float64(0),
						"num_column_mismatch": float64(0),
						"num_extraneous":      float64(0),
						"num_live_retry":      float64(0),
					},
				},
				"mismatch_tables": []any{
					map[string]any{
						"schema": "public",
						"table":  "orders",
					},
				},
				"table_definition_mismatches": []any{
					map[string]any{
						"schema":  "public",
						"table":   "orders",
						"message": "primary key mismatch",
					},
				},
			},
		}, payload)
		return true
	}, 2*time.Second, 20*time.Millisecond)
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
	require.Contains(t, string(body), "cockroach_migration_tool_verify_active_jobs 1")
	require.Contains(t, string(body), `cockroach_migration_tool_verify_jobs_total{status="running"} 1`)
}

func TestMetricsExposeOnlyCoarseLifecycleState(t *testing.T) {
	t.Parallel()

	runner := reportingRunner(func(ctx context.Context, reporter inconsistency.Reporter) error {
		reporter.Report(inconsistency.StatusReport{Info: "verifying public.accounts"})
		reporter.Report(inconsistency.SummaryReport{
			Info: "accounts summary",
			Stats: inconsistency.RowStats{
				Schema:      "public",
				Table:       "accounts",
				NumVerified: 7,
				NumMismatch: 2,
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
		require.Contains(t, metrics, "cockroach_migration_tool_verify_active_jobs 1")
		require.Contains(t, metrics, `cockroach_migration_tool_verify_jobs_total{status="running"} 1`)
		require.NotContains(t, metrics, "job_id")
		require.NotContains(t, metrics, "source_db")
		require.NotContains(t, metrics, "target_db")
		require.NotContains(t, metrics, "schema=")
		require.NotContains(t, metrics, "table=")
		require.NotContains(t, metrics, "kind=")
		require.NotContains(t, metrics, "accounts")
		require.NotContains(t, metrics, "public")
		return true
	}, 2*time.Second, 20*time.Millisecond)
}

func TestMetricsExposeFailedJobLifecycleState(t *testing.T) {
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
		return strings.Contains(metrics, "cockroach_migration_tool_verify_active_jobs 0") &&
			strings.Contains(metrics, `cockroach_migration_tool_verify_jobs_total{status="failed"} 1`)
	}, 2*time.Second, 20*time.Millisecond)
}

func TestCompletedJobRetentionOnlyKeepsMostRecentJob(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner:      reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
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

	require.Eventually(t, func() bool {
		response, err := http.Get(server.URL + "/jobs/job-000001")
		require.NoError(t, err)
		defer func() {
			_ = response.Body.Close()
		}()

		if response.StatusCode != http.StatusOK {
			return false
		}

		var payload map[string]any
		require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
		return payload["status"] == "succeeded"
	}, 2*time.Second, 20*time.Millisecond)

	secondResponse, err := http.Post(server.URL+"/jobs", "application/json", bytes.NewBufferString(`{}`))
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = secondResponse.Body.Close()
	})
	require.Equal(t, http.StatusAccepted, secondResponse.StatusCode)

	require.Eventually(t, func() bool {
		response, err := http.Get(server.URL + "/jobs/job-000002")
		require.NoError(t, err)
		defer func() {
			_ = response.Body.Close()
		}()

		if response.StatusCode != http.StatusOK {
			return false
		}

		var payload map[string]any
		require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
		return payload["status"] == "succeeded"
	}, 2*time.Second, 20*time.Millisecond)

	evictedResponse, err := http.Get(server.URL + "/jobs/job-000001")
	require.NoError(t, err)
	defer func() {
		_ = evictedResponse.Body.Close()
	}()
	require.Equal(t, http.StatusNotFound, evictedResponse.StatusCode)

	retainedResponse, err := http.Get(server.URL + "/jobs/job-000002")
	require.NoError(t, err)
	defer func() {
		_ = retainedResponse.Body.Close()
	}()
	require.Equal(t, http.StatusOK, retainedResponse.StatusCode)

	metricsResponse, err := http.Get(server.URL + "/metrics")
	require.NoError(t, err)
	defer func() {
		_ = metricsResponse.Body.Close()
	}()
	require.Equal(t, http.StatusOK, metricsResponse.StatusCode)

	metricsBody, err := io.ReadAll(metricsResponse.Body)
	require.NoError(t, err)
	metrics := string(metricsBody)
	require.Contains(t, metrics, "cockroach_migration_tool_verify_active_jobs 0")
	require.Contains(t, metrics, `cockroach_migration_tool_verify_jobs_total{status="succeeded"} 1`)
	require.NotContains(t, metrics, `cockroach_migration_tool_verify_jobs_total{status="succeeded"} 2`)
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
		if !strings.Contains(metrics, `cockroach_migration_tool_verify_jobs_total{status="failed"} 1`) {
			return false
		}

		parser := expfmt.TextParser{}
		families, err := parser.TextToMetricFamilies(strings.NewReader(metrics))
		require.NoError(t, err)
		require.Empty(t, metricLabelNames(t, families["cockroach_migration_tool_verify_active_jobs"]))
		require.Equal(t, []string{"status"}, metricLabelNames(t, families["cockroach_migration_tool_verify_jobs_total"]))
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

func TestPostTablesRawFailsClosedWhenDisabled(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(verifyservice.Config{}, verifyservice.Dependencies{
		Runner: reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/tables/raw",
		"application/json",
		bytes.NewBufferString(`{"database":"source","schema":"public","table":"accounts"}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusForbidden, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Equal(t, "raw table output is disabled", payload.Error)
}

func TestPostTablesRawReturnsSourceRowsWhenEnabled(t *testing.T) {
	t.Parallel()

	reader := &fakeRawTableReader{
		response: verifyservice.RawTableResponse{
			Database: "source",
			Schema:   "public",
			Table:    "accounts",
			Columns:  []string{"id", "email"},
			Rows: []map[string]any{
				{
					"id":    float64(1),
					"email": "first@example.com",
				},
			},
		},
	}
	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			RawTableOutput: verifyservice.RawTableOutputConfig{
				Enabled: true,
			},
		},
	}, verifyservice.Dependencies{
		Runner:         reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		RawTableReader: reader,
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/tables/raw",
		"application/json",
		bytes.NewBufferString(`{"database":"source","schema":"public","table":"accounts"}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusOK, response.StatusCode)

	var payload map[string]any
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Equal(t, map[string]any{
		"database": "source",
		"schema":   "public",
		"table":    "accounts",
		"columns":  []any{"id", "email"},
		"rows": []any{
			map[string]any{
				"id":    float64(1),
				"email": "first@example.com",
			},
		},
	}, payload)
	require.Equal(t, verifyservice.RawTableRequest{
		Database: "source",
		Schema:   "public",
		Table:    "accounts",
	}, reader.lastRequest)
}

func TestPostTablesRawReturnsDestinationRowsWhenEnabled(t *testing.T) {
	t.Parallel()

	reader := &fakeRawTableReader{
		response: verifyservice.RawTableResponse{
			Database: "destination",
			Schema:   "public",
			Table:    "accounts",
			Columns:  []string{"id", "email"},
			Rows: []map[string]any{
				{
					"id":    float64(9),
					"email": "target@example.com",
				},
			},
		},
	}
	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			RawTableOutput: verifyservice.RawTableOutputConfig{
				Enabled: true,
			},
		},
	}, verifyservice.Dependencies{
		Runner:         reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		RawTableReader: reader,
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/tables/raw",
		"application/json",
		bytes.NewBufferString(`{"database":"destination","schema":"public","table":"accounts"}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusOK, response.StatusCode)

	var payload map[string]any
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Equal(t, "destination", payload["database"])
	require.Equal(t, verifyservice.RawTableRequest{
		Database: "destination",
		Schema:   "public",
		Table:    "accounts",
	}, reader.lastRequest)
}

func TestPostTablesRawRejectsInvalidIdentifiers(t *testing.T) {
	t.Parallel()

	service := verifyservice.NewService(verifyservice.Config{
		Verify: verifyservice.VerifyConfig{
			RawTableOutput: verifyservice.RawTableOutputConfig{
				Enabled: true,
			},
		},
	}, verifyservice.Dependencies{
		Runner:         reportingRunner(func(_ context.Context, _ inconsistency.Reporter) error { return nil }),
		RawTableReader: &fakeRawTableReader{},
	})
	t.Cleanup(service.Close)

	server := httptest.NewServer(service.Handler())
	t.Cleanup(server.Close)

	response, err := http.Post(
		server.URL+"/tables/raw",
		"application/json",
		bytes.NewBufferString(`{"database":"source","schema":"public;drop schema public","table":"accounts"}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Equal(t, "schema must be a simple SQL identifier", payload.Error)
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

func TestGetJobReturnsOnlySafeStatusFieldsAfterFailure(t *testing.T) {
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

		body, err := io.ReadAll(getResponse.Body)
		require.NoError(t, err)

		var payload map[string]any
		require.NoError(t, json.Unmarshal(body, &payload))
		if payload["status"] != "failed" {
			return false
		}

		require.Equal(t, map[string]any{
			"job_id": "job-000001",
			"status": "failed",
		}, payload)
		require.NotContains(t, string(body), "table_definition")
		require.NotContains(t, string(body), "public")
		require.NotContains(t, string(body), "accounts")
		require.NotContains(t, string(body), "verify exploded")
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

func TestPostJobsRejectsConnectionLikeRequestFields(t *testing.T) {
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
	require.Equal(t, http.StatusBadRequest, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Contains(t, payload.Error, `json: unknown field "verify"`)

	select {
	case <-runner.started:
		t.Fatal("runner must not start when connection-like request fields are present")
	default:
	}
}

func TestPostJobsRejectsUnknownTopLevelFields(t *testing.T) {
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
					"schema": "^public$"
				}
			},
			"command": "sh -c 'curl attacker'"
		}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Contains(t, payload.Error, `json: unknown field "command"`)

	select {
	case <-runner.started:
		t.Fatal("runner must not start when unknown request fields are present")
	default:
	}
}

func TestPostJobsRejectsTrailingJSONDocuments(t *testing.T) {
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
		bytes.NewBufferString(`{}{"filters":{"include":{"schema":"^public$"}}}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusBadRequest, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.Contains(t, payload.Error, "request body must contain exactly one JSON object")

	select {
	case <-runner.started:
		t.Fatal("runner must not start when multiple JSON documents are present")
	default:
	}
}

func TestPostJobsRejectsOversizedRequestBody(t *testing.T) {
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
		bytes.NewBufferString(`{"filters":{"include":{"schema":"`+strings.Repeat("a", 1<<20)+`"}}}`),
	)
	require.NoError(t, err)
	t.Cleanup(func() {
		_ = response.Body.Close()
	})

	require.Equal(t, http.StatusRequestEntityTooLarge, response.StatusCode)

	var payload struct {
		Error string `json:"error"`
	}
	require.NoError(t, json.NewDecoder(response.Body).Decode(&payload))
	require.NotEmpty(t, payload.Error)

	select {
	case <-runner.started:
		t.Fatal("runner must not start when request body exceeds the limit")
	default:
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

type fakeRawTableReader struct {
	lastRequest verifyservice.RawTableRequest
	response    verifyservice.RawTableResponse
	err         error
}

func (r *fakeRawTableReader) ReadRawTable(_ context.Context, request verifyservice.RawTableRequest) (verifyservice.RawTableResponse, error) {
	r.lastRequest = request
	return r.response, r.err
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
