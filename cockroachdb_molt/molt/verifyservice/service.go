package verifyservice

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/cockroachdb/molt/verify/inconsistency"
)

type JobStatus string

const (
	JobStatusRunning   JobStatus = "running"
	JobStatusSucceeded JobStatus = "succeeded"
	JobStatusFailed    JobStatus = "failed"
	JobStatusStopped   JobStatus = "stopped"

	verifyRequestBodyMaxBytes = 64 << 10
)

type Runner interface {
	Run(ctx context.Context, request RunRequest, reporter inconsistency.Reporter) error
}

type Dependencies struct {
	Runner      Runner
	IDGenerator func() string
	Now         func() time.Time
}

type Service struct {
	mu          sync.Mutex
	runner      Runner
	idGenerator func() string
	now         func() time.Time
	jobs        map[string]*job
	activeJobID string
}

type job struct {
	id            string
	status        JobStatus
	startedAt     time.Time
	finishedAt    *time.Time
	cancel        context.CancelFunc
	failureReason *string
}

type jobStatusView struct {
	JobID  string    `json:"job_id"`
	Status JobStatus `json:"status"`
}

func NewService(cfg Config, deps Dependencies) *Service {
	if deps.Runner == nil {
		panic("verifyservice.Dependencies.Runner must be set")
	}
	if deps.IDGenerator == nil {
		deps.IDGenerator = newSequentialJobIDGenerator()
	}
	if deps.Now == nil {
		deps.Now = time.Now
	}
	return &Service{
		runner:      deps.Runner,
		idGenerator: deps.IDGenerator,
		now:         deps.Now,
		jobs:        make(map[string]*job),
	}
}

func (s *Service) Handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("POST /jobs", s.handlePostJobs)
	mux.HandleFunc("GET /jobs/{job_id}", s.handleGetJob)
	mux.HandleFunc("POST /stop", s.handlePostStop)
	mux.Handle("GET /metrics", newMetricsHandler(s))
	return mux
}

func (s *Service) Close() {
	s.mu.Lock()
	cancel := s.activeCancelLocked()
	s.mu.Unlock()
	if cancel != nil {
		cancel()
	}
}

func (s *Service) handlePostJobs(w http.ResponseWriter, r *http.Request) {
	var jobRequest JobRequest
	if err := decodeJSONBody(w, r, &jobRequest); err != nil {
		writeDecodeJSONError(w, err)
		return
	}
	runRequest, err := jobRequest.Compile()
	if err != nil {
		writeJSONError(w, http.StatusBadRequest, err.Error())
		return
	}

	job, err := s.startJob(runRequest)
	if err != nil {
		if errors.Is(err, errJobAlreadyRunning) {
			writeJSONError(w, http.StatusConflict, err.Error())
			return
		}
		writeJSONError(w, http.StatusInternalServerError, err.Error())
		return
	}

	writeJSON(w, http.StatusAccepted, struct {
		JobID  string    `json:"job_id"`
		Status JobStatus `json:"status"`
	}{
		JobID:  job.id,
		Status: job.status,
	})
}

var errJobAlreadyRunning = errors.New("a verify job is already running")

func (s *Service) startJob(request RunRequest) (*job, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.activeJobID != "" {
		return nil, errJobAlreadyRunning
	}

	ctx, cancel := context.WithCancel(context.Background())
	job := &job{
		id:        s.idGenerator(),
		status:    JobStatusRunning,
		startedAt: s.now(),
		cancel:    cancel,
	}
	s.jobs[job.id] = job
	s.activeJobID = job.id

	go func() {
		err := s.runner.Run(ctx, request, jobReporter{service: s, jobID: job.id})
		s.finishJob(job.id, err)
	}()

	return job, nil
}

func (s *Service) finishJob(jobID string, err error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	job, ok := s.jobs[jobID]
	if !ok {
		return
	}

	switch {
	case err == nil:
		job.status = JobStatusSucceeded
	case errors.Is(err, context.Canceled):
		job.status = JobStatusStopped
		failureReason := "job stopped by request"
		job.failureReason = &failureReason
	default:
		job.status = JobStatusFailed
		failureReason := err.Error()
		job.failureReason = &failureReason
	}

	if s.activeJobID == jobID {
		s.activeJobID = ""
	}
	finishedAt := s.now()
	job.finishedAt = &finishedAt
	job.cancel = nil
}

func (s *Service) handleGetJob(w http.ResponseWriter, r *http.Request) {
	jobID := r.PathValue("job_id")
	jobResponse, ok := s.getJobResponse(jobID)
	if !ok {
		writeJSONError(w, http.StatusNotFound, "job not found")
		return
	}
	writeJSON(w, http.StatusOK, jobResponse)
}

func (s *Service) handlePostStop(w http.ResponseWriter, r *http.Request) {
	var request struct {
		JobID string `json:"job_id"`
	}
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeDecodeJSONError(w, err)
		return
	}

	var stoppedJobIDs []string
	var err error
	if request.JobID == "" {
		stoppedJobIDs = s.stopAllJobs()
	} else {
		stoppedJobIDs, err = s.stopJob(request.JobID)
		if err != nil {
			if errors.Is(err, errJobNotFound) {
				writeJSONError(w, http.StatusNotFound, err.Error())
				return
			}
			writeJSONError(w, http.StatusInternalServerError, err.Error())
			return
		}
	}

	writeJSON(w, http.StatusOK, struct {
		StoppedJobIDs []string `json:"stopped_job_ids"`
	}{
		StoppedJobIDs: stoppedJobIDs,
	})
}

func (s *Service) getJobResponse(jobID string) (any, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	storedJob, ok := s.jobs[jobID]
	if !ok {
		return nil, false
	}
	return storedJob.response(), true
}

var errJobNotFound = errors.New("job not found")

func (s *Service) stopAllJobs() []string {
	s.mu.Lock()
	cancel := s.activeCancelLocked()
	activeJobID := s.activeJobID
	s.mu.Unlock()

	if cancel == nil || activeJobID == "" {
		return nil
	}
	cancel()
	return []string{activeJobID}
}

func (s *Service) stopJob(jobID string) ([]string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.activeJobID != jobID {
		return nil, errJobNotFound
	}
	activeJob := s.jobs[jobID]
	if activeJob == nil || activeJob.cancel == nil {
		return nil, errJobNotFound
	}
	activeJob.cancel()
	return []string{jobID}, nil
}

func (j job) response() any {
	return jobStatusView{
		JobID:  j.id,
		Status: j.status,
	}
}

func (s *Service) activeCancelLocked() context.CancelFunc {
	if s.activeJobID == "" {
		return nil
	}
	activeJob := s.jobs[s.activeJobID]
	if activeJob == nil {
		return nil
	}
	return activeJob.cancel
}

func writeJSON(w http.ResponseWriter, status int, payload any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(payload); err != nil {
		panic(err)
	}
}

func writeJSONError(w http.ResponseWriter, status int, message string) {
	writeJSON(w, status, struct {
		Error string `json:"error"`
	}{
		Error: message,
	})
}

var errRequestBodyTooLarge = errors.New("request body exceeds maximum size")

func writeDecodeJSONError(w http.ResponseWriter, err error) {
	if errors.Is(err, errRequestBodyTooLarge) {
		writeJSONError(w, http.StatusRequestEntityTooLarge, err.Error())
		return
	}
	writeJSONError(w, http.StatusBadRequest, err.Error())
}

func decodeJSONBody(w http.ResponseWriter, r *http.Request, destination any) error {
	decoder := json.NewDecoder(http.MaxBytesReader(w, r.Body, verifyRequestBodyMaxBytes))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return normalizeDecodeJSONError(err)
	}
	var extraDocument any
	if err := decoder.Decode(&extraDocument); err != io.EOF {
		if err != nil {
			return normalizeDecodeJSONError(err)
		}
		return errors.New("request body must contain exactly one JSON object")
	}
	return nil
}

func normalizeDecodeJSONError(err error) error {
	var maxBytesErr *http.MaxBytesError
	if errors.As(err, &maxBytesErr) {
		return errRequestBodyTooLarge
	}
	return err
}

type jobReporter struct {
	service *Service
	jobID   string
}

func (r jobReporter) Report(obj inconsistency.ReportableObject) {
	r.service.recordReport(r.jobID, obj)
}

func (jobReporter) Close() {}

func newSequentialJobIDGenerator() func() string {
	var (
		mu      sync.Mutex
		counter int
	)
	return func() string {
		mu.Lock()
		defer mu.Unlock()
		counter++
		return "job-" + strings.Repeat("0", max(0, 6-len(strconv.Itoa(counter)))) + strconv.Itoa(counter)
	}
}

func max(left int, right int) int {
	if left > right {
		return left
	}
	return right
}

func (s *Service) recordReport(jobID string, obj inconsistency.ReportableObject) {
	_ = jobID
	_ = obj
}
