package verifyservice

import (
	"net/http"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

const (
	verifyMetricsPrefix = "cockroach_migration_tool_verify_"
)

var jobStateDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"job_state",
	"Current state for each verify job.",
	[]string{"job_id", "status"},
	nil,
)

var sourceRowsDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"source_rows_total",
	"Cumulative source-side row count for each verified table.",
	[]string{"job_id", "database", "schema", "table"},
	nil,
)

var checkedRowsDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"checked_rows_total",
	"Cumulative checked row count for each verified table.",
	[]string{"job_id", "schema", "table"},
	nil,
)

var destinationRowsDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"destination_rows_total",
	"Cumulative destination-side row count for each verified table.",
	[]string{"job_id", "database", "schema", "table"},
	nil,
)

var mismatchesDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"mismatches_total",
	"Cumulative mismatch totals for each verified table and mismatch kind.",
	[]string{"job_id", "schema", "table", "kind"},
	nil,
)

var errorsDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"errors_total",
	"Cumulative runtime error count for each verify job.",
	[]string{"job_id"},
	nil,
)

type serviceMetricsCollector struct {
	service *Service
}

type metricsJobSnapshot struct {
	id       string
	status   JobStatus
	progress jobProgressSnapshot
}

func newMetricsHandler(service *Service) http.Handler {
	registry := prometheus.NewRegistry()
	if err := registry.Register(serviceMetricsCollector{service: service}); err != nil {
		panic(err)
	}
	return promhttp.HandlerFor(registry, promhttp.HandlerOpts{})
}

func (c serviceMetricsCollector) Describe(ch chan<- *prometheus.Desc) {
	ch <- jobStateDesc
	ch <- sourceRowsDesc
	ch <- checkedRowsDesc
	ch <- destinationRowsDesc
	ch <- mismatchesDesc
	ch <- errorsDesc
}

func (c serviceMetricsCollector) Collect(ch chan<- prometheus.Metric) {
	for _, job := range c.service.metricsJobSnapshots() {
		ch <- prometheus.MustNewConstMetric(
			jobStateDesc,
			prometheus.GaugeValue,
			1,
			job.id,
			string(job.status),
		)
		ch <- prometheus.MustNewConstMetric(
			errorsDesc,
			prometheus.GaugeValue,
			job.progress.errorCount(),
			job.id,
		)
		for _, table := range job.progress.sortedTables() {
			ch <- prometheus.MustNewConstMetric(
				sourceRowsDesc,
				prometheus.GaugeValue,
				table.sourceRows(),
				job.id,
				c.service.sourceDB,
				table.schema(),
				table.table(),
			)
			ch <- prometheus.MustNewConstMetric(
				checkedRowsDesc,
				prometheus.GaugeValue,
				table.checkedRows(),
				job.id,
				table.schema(),
				table.table(),
			)
			ch <- prometheus.MustNewConstMetric(
				destinationRowsDesc,
				prometheus.GaugeValue,
				table.destinationRows(),
				job.id,
				c.service.targetDB,
				table.schema(),
				table.table(),
			)
			for kind, count := range table.mismatchKinds() {
				ch <- prometheus.MustNewConstMetric(
					mismatchesDesc,
					prometheus.GaugeValue,
					count,
					job.id,
					table.schema(),
					table.table(),
					kind,
				)
			}
		}
	}
}

func (s *Service) metricsJobSnapshots() []metricsJobSnapshot {
	s.mu.Lock()
	defer s.mu.Unlock()

	snapshots := make([]metricsJobSnapshot, 0, len(s.jobs))
	for _, job := range s.jobs {
		snapshots = append(snapshots, metricsJobSnapshot{
			id:       job.id,
			status:   job.status,
			progress: job.progress.copy(),
		})
	}
	return snapshots
}
