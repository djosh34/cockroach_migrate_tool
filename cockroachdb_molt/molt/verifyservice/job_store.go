package verifyservice

import (
	"context"
	"slices"
	"sync"
	"time"
)

type jobStore struct {
	mu    sync.Mutex
	jobs  map[string]*job
	order []string
}

func newJobStore() *jobStore {
	return &jobStore{
		jobs: make(map[string]*job),
	}
}

func (s *jobStore) add(job *job) {
	s.mu.Lock()
	defer s.mu.Unlock()

	s.jobs[job.id] = job
	s.order = append(s.order, job.id)
}

func (s *jobStore) get(jobID string) (jobStatusView, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return jobStatusView{}, false
	}
	return job.response(), true
}

func (s *jobStore) list() []jobStatusView {
	s.mu.Lock()
	defer s.mu.Unlock()

	responses := make([]jobStatusView, 0, len(s.order))
	for _, jobID := range s.order {
		job := s.jobs[jobID]
		if job == nil {
			continue
		}
		responses = append(responses, job.response())
	}
	return responses
}

func (s *jobStore) stop(jobID string) (jobStatusView, context.CancelFunc, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil || !job.active() || job.cancel == nil {
		return jobStatusView{}, nil, errJobNotFound
	}

	job.markStopping()
	return job.response(), job.cancel, nil
}

func (s *jobStore) activeCancels() []context.CancelFunc {
	s.mu.Lock()
	defer s.mu.Unlock()

	cancels := make([]context.CancelFunc, 0)
	for _, jobID := range s.order {
		job := s.jobs[jobID]
		if job == nil || job.cancel == nil || !job.active() {
			continue
		}
		cancels = append(cancels, job.cancel)
	}
	return cancels
}

func (s *jobStore) recordReport(jobID string, databaseName string, obj any) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return
	}
	database := job.databaseNamed(databaseName)
	if database == nil {
		return
	}
	database.recordReport(obj)
}

func (s *jobStore) completeDatabase(jobID string, databaseName string, now time.Time) *operatorError {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return nil
	}
	database := job.databaseNamed(databaseName)
	if database == nil {
		return nil
	}
	return database.complete(now)
}

func (s *jobStore) failDatabase(jobID string, databaseName string, now time.Time, err *operatorError) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return
	}
	database := job.databaseNamed(databaseName)
	if database == nil {
		return
	}
	database.fail(now, err)
}

func (s *jobStore) finishJob(jobID string, now time.Time) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return
	}
	job.finish(now)
}

func (s *jobStore) stopJob(jobID string, now time.Time) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobs[jobID]
	if job == nil {
		return
	}
	job.stop(now)
}

func (s *jobStore) metricsStatusSnapshot() metricsStatusSnapshot {
	s.mu.Lock()
	defer s.mu.Unlock()

	snapshot := metricsStatusSnapshot{
		statusCounts: map[JobStatus]float64{
			JobStatusRunning:   0,
			JobStatusStopping:  0,
			JobStatusSucceeded: 0,
			JobStatusFailed:    0,
			JobStatusStopped:   0,
		},
	}

	for _, jobID := range s.order {
		job := s.jobs[jobID]
		if job == nil {
			continue
		}
		status := job.status()
		snapshot.statusCounts[status]++
		if job.active() {
			snapshot.activeJobs++
		}
	}
	return snapshot
}

func (s *jobStore) orderedJobIDs() []string {
	s.mu.Lock()
	defer s.mu.Unlock()
	return slices.Clone(s.order)
}
