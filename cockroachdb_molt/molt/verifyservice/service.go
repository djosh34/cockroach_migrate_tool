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

	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/rs/zerolog"
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
	Runner         Runner
	IDGenerator    func() string
	RawTableReader RawTableReader
	Logger         zerolog.Logger
}

type Service struct {
	mu               sync.Mutex
	verifyConfig     VerifyConfig
	runner           Runner
	idGenerator      func() string
	logger           zerolog.Logger
	rawTableEnabled  bool
	rawTableReader   RawTableReader
	activeJob        *job
	lastCompletedJob *job
}

func NewService(cfg Config, deps Dependencies) *Service {
	if deps.Runner == nil {
		panic("verifyservice.Dependencies.Runner must be set")
	}
	if deps.IDGenerator == nil {
		deps.IDGenerator = newSequentialJobIDGenerator()
	}
	if deps.RawTableReader == nil {
		deps.RawTableReader = newConfigBackedRawTableReader(cfg)
	}
	return &Service{
		verifyConfig:    cfg.Verify,
		runner:          deps.Runner,
		idGenerator:     deps.IDGenerator,
		logger:          deps.Logger,
		rawTableEnabled: cfg.Verify.RawTableOutput,
		rawTableReader:  deps.RawTableReader,
	}
}

func (s *Service) Handler() http.Handler {
	mux := http.NewServeMux()
	mux.HandleFunc("POST /jobs", s.handlePostJobs)
	mux.HandleFunc("GET /jobs/{job_id}", s.handleGetJob)
	mux.HandleFunc("POST /jobs/{job_id}/stop", s.handlePostJobStop)
	mux.HandleFunc("POST /tables/raw", s.handlePostTablesRaw)
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
		writeOperatorError(w, http.StatusBadRequest, err)
		return
	}
	if err := runRequest.ValidateSelection(s.verifyConfig); err != nil {
		writeOperatorError(w, http.StatusBadRequest, err)
		return
	}

	job, err := s.startJob(runRequest)
	if err != nil {
		if errors.Is(err, errJobAlreadyRunning) {
			writeOperatorError(w, http.StatusConflict, err)
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

var errJobAlreadyRunning = newOperatorError("job_state", "job_already_running", "a verify job is already running")

func (s *Service) startJob(request RunRequest) (*job, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.activeJob != nil {
		return nil, errJobAlreadyRunning
	}

	ctx, cancel := context.WithCancel(context.Background())
	job := newJob(s.idGenerator(), cancel)
	s.activeJob = job

	go func() {
		err := s.runner.Run(ctx, request, jobReporter{service: s, jobID: job.id})
		s.finishJob(job.id, err)
	}()

	return job, nil
}

func (s *Service) finishJob(jobID string, err error) {
	s.mu.Lock()

	job := s.activeJob
	if job == nil || job.id != jobID {
		s.mu.Unlock()
		return
	}

	var failureToLog *operatorError
	switch {
	case err == nil:
		if mismatchFailure := job.result.mismatchFailure(); mismatchFailure != nil {
			job.status = JobStatusFailed
			job.failure = mismatchFailure
			failureToLog = mismatchFailure
		} else {
			job.status = JobStatusSucceeded
		}
	case errors.Is(err, context.Canceled):
		job.status = JobStatusStopped
	default:
		job.status = JobStatusFailed
		job.failure = classifyRunFailure(err)
		failureToLog = job.failure
	}
	job.cancel = nil
	s.lastCompletedJob = job
	s.activeJob = nil
	logger := s.logger
	s.mu.Unlock()

	if failureToLog != nil {
		logJobFailure(logger, failureToLog)
	}
}

func logJobFailure(logger zerolog.Logger, failure *operatorError) {
	view := failure.view()
	event := logger.Error().
		Str("event", "job.failed").
		Str("category", view.Category).
		Str("code", view.Code)
	if len(view.Details) > 0 {
		event = event.Any("details", view.Details)
	}
	event.Msg(view.Message)
}

func (s *Service) handleGetJob(w http.ResponseWriter, r *http.Request) {
	jobID := r.PathValue("job_id")
	jobResponse, ok := s.getJobResponse(jobID)
	if !ok {
		writeOperatorError(w, http.StatusNotFound, errJobNotFound)
		return
	}
	writeJSON(w, http.StatusOK, jobResponse)
}

func (s *Service) handlePostJobStop(w http.ResponseWriter, r *http.Request) {
	var request struct{}
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeDecodeJSONError(w, err)
		return
	}

	jobID := r.PathValue("job_id")
	if err := s.stopJob(jobID); err != nil {
		if errors.Is(err, errJobNotFound) {
			writeOperatorError(w, http.StatusNotFound, err)
			return
		}
		writeJSONError(w, http.StatusInternalServerError, err.Error())
		return
	}

	writeJSON(w, http.StatusOK, struct {
		JobID  string `json:"job_id"`
		Status string `json:"status"`
	}{
		JobID:  jobID,
		Status: "stopping",
	})
}

func (s *Service) handlePostTablesRaw(w http.ResponseWriter, r *http.Request) {
	if !s.rawTableEnabled {
		writeJSONError(w, http.StatusForbidden, "raw table output is disabled")
		return
	}
	var request RawTableRequest
	if err := decodeJSONBody(w, r, &request); err != nil {
		writeDecodeJSONError(w, err)
		return
	}
	if err := request.Validate(); err != nil {
		writeJSONError(w, http.StatusBadRequest, err.Error())
		return
	}
	response, err := s.rawTableReader.ReadRawTable(r.Context(), request)
	if err != nil {
		var requestErr rawTableRequestError
		if errors.As(err, &requestErr) {
			writeJSONError(w, http.StatusBadRequest, err.Error())
			return
		}
		writeJSONError(w, http.StatusInternalServerError, err.Error())
		return
	}
	writeJSON(w, http.StatusOK, response)
}

func (s *Service) getJobResponse(jobID string) (any, bool) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.activeJob != nil && s.activeJob.id == jobID {
		return s.activeJob.response(), true
	}
	if s.lastCompletedJob != nil && s.lastCompletedJob.id == jobID {
		return s.lastCompletedJob.response(), true
	}
	return nil, false
}

var errJobNotFound = newOperatorError("job_state", "job_not_found", "job not found")

func (s *Service) stopJob(jobID string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.activeJob == nil || s.activeJob.id != jobID {
		return errJobNotFound
	}
	if s.activeJob.cancel == nil {
		return errJobNotFound
	}
	s.activeJob.cancel()
	return nil
}

func (s *Service) activeCancelLocked() context.CancelFunc {
	if s.activeJob == nil {
		return nil
	}
	return s.activeJob.cancel
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

func writeOperatorError(w http.ResponseWriter, status int, err error) {
	if opErr, ok := asOperatorError(err); ok {
		writeJSON(w, status, operatorErrorResponse{
			Error: opErr.payload(),
		})
		return
	}
	writeJSONError(w, status, err.Error())
}

var errRequestBodyTooLarge = errors.New("request body exceeds maximum size")

func writeDecodeJSONError(w http.ResponseWriter, err error) {
	if errors.Is(err, errRequestBodyTooLarge) {
		writeOperatorError(w, http.StatusRequestEntityTooLarge, classifyDecodeJSONError(err))
		return
	}
	writeOperatorError(w, http.StatusBadRequest, classifyDecodeJSONError(err))
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
	s.mu.Lock()
	defer s.mu.Unlock()

	job := s.jobLocked(jobID)
	if job == nil {
		return
	}
	job.recordReport(obj)
}

func (s *Service) jobLocked(jobID string) *job {
	if s.activeJob != nil && s.activeJob.id == jobID {
		return s.activeJob
	}
	if s.lastCompletedJob != nil && s.lastCompletedJob.id == jobID {
		return s.lastCompletedJob
	}
	return nil
}
