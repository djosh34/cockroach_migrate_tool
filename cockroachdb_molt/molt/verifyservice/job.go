package verifyservice

import (
	"context"
	"slices"
	"time"

	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
)

type JobStatus string

const (
	JobStatusRunning   JobStatus = "running"
	JobStatusStopping  JobStatus = "stopping"
	JobStatusSucceeded JobStatus = "succeeded"
	JobStatusFailed    JobStatus = "failed"
	JobStatusStopped   JobStatus = "stopped"
)

type job struct {
	id         string
	createdAt  time.Time
	startedAt  time.Time
	finishedAt *time.Time
	databases  []*databaseJob
	cancel     context.CancelFunc
}

type databaseJob struct {
	name        string
	status      JobStatus
	startedAt   time.Time
	finishedAt  *time.Time
	schemas     []string
	tables      []string
	rowsChecked *int
	err         *operatorError
	result      jobResult
}

type jobStatusView struct {
	JobID      string               `json:"job_id"`
	Status     JobStatus            `json:"status"`
	CreatedAt  time.Time            `json:"created_at"`
	StartedAt  time.Time            `json:"started_at"`
	FinishedAt *time.Time           `json:"finished_at"`
	Databases  []databaseStatusView `json:"databases"`
}

type databaseStatusView struct {
	Name        string                `json:"name"`
	Status      JobStatus             `json:"status"`
	StartedAt   time.Time             `json:"started_at"`
	FinishedAt  *time.Time            `json:"finished_at"`
	Schemas     []string              `json:"schemas"`
	Tables      []string              `json:"tables"`
	RowsChecked *int                  `json:"rows_checked"`
	Error       *operatorErrorPayload `json:"error"`
	Findings    []findingView         `json:"findings"`
}

func newJob(id string, plan ResolvedJobPlan, now time.Time, cancel context.CancelFunc) *job {
	databases := make([]*databaseJob, 0, len(plan.Databases))
	for _, databasePlan := range plan.Databases {
		databases = append(databases, newDatabaseJob(databasePlan, now))
	}

	return &job{
		id:        id,
		createdAt: now,
		startedAt: now,
		databases: databases,
		cancel:    cancel,
	}
}

func newDatabaseJob(plan ResolvedDatabasePlan, now time.Time) *databaseJob {
	return &databaseJob{
		name:      plan.Database.Name,
		status:    JobStatusRunning,
		startedAt: now,
		schemas:   slices.Clone(plan.InitialSchemas),
		result:    newJobResult(),
	}
}

func (j *job) response() jobStatusView {
	databases := make([]databaseStatusView, 0, len(j.databases))
	for _, database := range j.databases {
		databases = append(databases, database.response())
	}

	return jobStatusView{
		JobID:      j.id,
		Status:     j.status(),
		CreatedAt:  j.createdAt,
		StartedAt:  j.startedAt,
		FinishedAt: cloneTimePointer(j.finishedAt),
		Databases:  databases,
	}
}

func (j *job) status() JobStatus {
	hasStopping := false
	hasRunning := false
	hasFailed := false
	hasStopped := false
	allSucceeded := true

	for _, database := range j.databases {
		switch database.status {
		case JobStatusStopping:
			hasStopping = true
			allSucceeded = false
		case JobStatusRunning:
			hasRunning = true
			allSucceeded = false
		case JobStatusFailed:
			hasFailed = true
			allSucceeded = false
		case JobStatusStopped:
			hasStopped = true
			allSucceeded = false
		case JobStatusSucceeded:
		}
	}

	switch {
	case hasStopping:
		return JobStatusStopping
	case hasRunning:
		return JobStatusRunning
	case allSucceeded:
		return JobStatusSucceeded
	case hasFailed:
		return JobStatusFailed
	case hasStopped:
		return JobStatusStopped
	default:
		return JobStatusSucceeded
	}
}

func (j *job) active() bool {
	switch j.status() {
	case JobStatusRunning, JobStatusStopping:
		return true
	default:
		return false
	}
}

func (j *job) databaseNamed(name string) *databaseJob {
	for _, database := range j.databases {
		if database.name == name {
			return database
		}
	}
	return nil
}

func (j *job) markStopping() {
	for _, database := range j.databases {
		database.markStopping()
	}
}

func (j *job) finish(now time.Time) {
	j.finishedAt = cloneTimePointer(&now)
	j.cancel = nil
}

func (j *job) stop(now time.Time) {
	for _, database := range j.databases {
		database.markStopped(now)
	}
	j.finish(now)
}

func (d *databaseJob) response() databaseStatusView {
	var errPayload *operatorErrorPayload
	if d.err != nil {
		payload := d.err.payload()
		errPayload = &payload
	}

	findings := d.result.findingsView()
	if len(findings) == 0 {
		findings = nil
	}

	return databaseStatusView{
		Name:        d.name,
		Status:      d.status,
		StartedAt:   d.startedAt,
		FinishedAt:  cloneTimePointer(d.finishedAt),
		Schemas:     cloneOrNil(d.schemas),
		Tables:      cloneOrNil(d.tables),
		RowsChecked: cloneIntPointer(d.rowsChecked),
		Error:       errPayload,
		Findings:    findings,
	}
}

func (d *databaseJob) recordReport(obj any) {
	d.result.recordReport(obj)

	schema, table, ok := reportLocation(obj)
	if ok {
		d.schemas = appendUniqueStrings(d.schemas, schema)
		d.tables = appendUniqueStrings(d.tables, table)
	}

	if summaryReport, ok := obj.(inconsistency.SummaryReport); ok {
		d.addRowsChecked(summaryReport.Stats.NumVerified)
	}
}

func (d *databaseJob) addRowsChecked(delta int) {
	if d.rowsChecked == nil {
		initial := 0
		d.rowsChecked = &initial
	}
	*d.rowsChecked += delta
}

func (d *databaseJob) complete(now time.Time) *operatorError {
	if mismatchFailure := d.result.mismatchFailure(); mismatchFailure != nil {
		d.status = JobStatusFailed
		d.err = mismatchFailure
		d.finishedAt = cloneTimePointer(&now)
		return mismatchFailure
	}

	d.status = JobStatusSucceeded
	d.finishedAt = cloneTimePointer(&now)
	return nil
}

func (d *databaseJob) fail(now time.Time, err *operatorError) {
	d.status = JobStatusFailed
	d.err = err
	d.finishedAt = cloneTimePointer(&now)
}

func (d *databaseJob) markStopping() {
	if d.status == JobStatusRunning {
		d.status = JobStatusStopping
	}
}

func (d *databaseJob) markStopped(now time.Time) {
	if d.status != JobStatusRunning && d.status != JobStatusStopping {
		return
	}
	d.status = JobStatusStopped
	d.finishedAt = cloneTimePointer(&now)
}

func cloneTimePointer(value *time.Time) *time.Time {
	if value == nil {
		return nil
	}
	cloned := *value
	return &cloned
}

func cloneIntPointer(value *int) *int {
	if value == nil {
		return nil
	}
	cloned := *value
	return &cloned
}

func cloneOrNil(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	return slices.Clone(values)
}

func reportLocation(obj any) (string, string, bool) {
	switch report := obj.(type) {
	case inconsistency.SummaryReport:
		return report.Stats.Schema, report.Stats.Table, true
	case inconsistency.MismatchingTableDefinition:
		return string(report.Schema), string(report.Table), true
	case inconsistency.MismatchingRow:
		return string(report.Schema), string(report.Table), true
	case inconsistency.MismatchingColumn:
		return string(report.Schema), string(report.Table), true
	case inconsistency.MissingRow:
		return string(report.Schema), string(report.Table), true
	case inconsistency.ExtraneousRow:
		return string(report.Schema), string(report.Table), true
	case utils.MissingTable:
		return string(report.Schema), string(report.Table), true
	case utils.ExtraneousTable:
		return string(report.Schema), string(report.Table), true
	default:
		return "", "", false
	}
}
