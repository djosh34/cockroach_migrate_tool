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
	verifyMetricsPrefix+"active_jobs",
	"Current number of active verify jobs.",
	nil,
	nil,
)

var jobsTotalDesc = prometheus.NewDesc(
	verifyMetricsPrefix+"jobs_total",
	"Current number of verify jobs by lifecycle status.",
	[]string{"status"},
	nil,
)

type serviceMetricsCollector struct {
	service *Service
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
	ch <- jobsTotalDesc
}

func (c serviceMetricsCollector) Collect(ch chan<- prometheus.Metric) {
	snapshot := c.service.metricsStatusSnapshot()
	ch <- prometheus.MustNewConstMetric(
		jobStateDesc,
		prometheus.GaugeValue,
		snapshot.activeJobs,
	)
	for _, status := range []JobStatus{
		JobStatusRunning,
		JobStatusSucceeded,
		JobStatusFailed,
		JobStatusStopped,
	} {
		ch <- prometheus.MustNewConstMetric(
			jobsTotalDesc,
			prometheus.GaugeValue,
			snapshot.statusCounts[status],
			string(status),
		)
	}
}

type metricsStatusSnapshot struct {
	activeJobs   float64
	statusCounts map[JobStatus]float64
}

func (s *Service) metricsStatusSnapshot() metricsStatusSnapshot {
	s.mu.Lock()
	defer s.mu.Unlock()

	snapshot := metricsStatusSnapshot{
		statusCounts: map[JobStatus]float64{
			JobStatusRunning:   0,
			JobStatusSucceeded: 0,
			JobStatusFailed:    0,
			JobStatusStopped:   0,
		},
	}
	if s.activeJobID != "" {
		snapshot.activeJobs = 1
	}
	for _, job := range s.jobs {
		snapshot.statusCounts[job.status]++
	}
	return snapshot
}
