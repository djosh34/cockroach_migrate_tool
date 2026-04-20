package verifyservice

import "context"

type job struct {
	id      string
	status  JobStatus
	cancel  context.CancelFunc
	result  jobResult
	failure *operatorError
}

func newJob(id string, cancel context.CancelFunc) *job {
	return &job{
		id:     id,
		status: JobStatusRunning,
		cancel: cancel,
		result: newJobResult(),
	}
}

func (j job) response() any {
	view := jobStatusView{
		JobID:  j.id,
		Status: j.status,
	}
	if j.status != JobStatusRunning && j.result.hasData() {
		view.Result = j.result.response()
	}
	if j.failure != nil {
		payload := j.failure.payload()
		view.Failure = &payload
	}
	return view
}

func (j *job) recordReport(obj any) {
	j.result.recordReport(obj)
}

type jobStatusView struct {
	JobID   string                `json:"job_id"`
	Status  JobStatus             `json:"status"`
	Result  *jobResultView        `json:"result,omitempty"`
	Failure *operatorErrorPayload `json:"failure,omitempty"`
}
