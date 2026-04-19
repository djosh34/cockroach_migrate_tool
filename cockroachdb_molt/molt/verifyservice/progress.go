package verifyservice

import (
	"sort"

	"github.com/cockroachdb/molt/verify/inconsistency"
)

type tableKey struct {
	schema string
	table  string
}

type tableProgress struct {
	summaryInfo               string
	stats                     rowStatsDTO
	tableDefinitionMismatches int
}

type jobProgressSnapshot struct {
	statusMessages []jobStatusMessage
	summaryEvents  []jobSummary
	tables         map[tableKey]tableProgress
	mismatches     []jobMismatch
	errors         []string
}

func newJobProgressSnapshot() jobProgressSnapshot {
	return jobProgressSnapshot{
		statusMessages: []jobStatusMessage{},
		summaryEvents:  []jobSummary{},
		tables:         make(map[tableKey]tableProgress),
		mismatches:     []jobMismatch{},
		errors:         []string{},
	}
}

func (p *jobProgressSnapshot) record(obj inconsistency.ReportableObject) {
	switch reported := obj.(type) {
	case inconsistency.StatusReport:
		p.statusMessages = append(p.statusMessages, jobStatusMessage{Info: reported.Info})
	case inconsistency.SummaryReport:
		p.summaryEvents = append(p.summaryEvents, jobSummary{
			Info:  reported.Info,
			Stats: toRowStatsDTO(reported.Stats),
		})
		key := tableKey{schema: reported.Stats.Schema, table: reported.Stats.Table}
		progress := p.tables[key]
		progress.summaryInfo = reported.Info
		progress.stats = toRowStatsDTO(reported.Stats)
		p.tables[key] = progress
	case inconsistency.MismatchingTableDefinition:
		key := tableKey{schema: string(reported.Schema), table: string(reported.Table)}
		progress := p.tables[key]
		progress.stats.Schema = key.schema
		progress.stats.Table = key.table
		progress.tableDefinitionMismatches++
		p.tables[key] = progress
		p.mismatches = append(p.mismatches, jobMismatch{
			Kind:   "table_definition",
			Schema: key.schema,
			Table:  key.table,
			Info:   reported.Info,
		})
	}
}

func (p *jobProgressSnapshot) recordError(message string) {
	p.errors = append(p.errors, message)
}

func (p jobProgressSnapshot) copy() jobProgressSnapshot {
	tables := make(map[tableKey]tableProgress, len(p.tables))
	for key, progress := range p.tables {
		tables[key] = progress
	}
	return jobProgressSnapshot{
		statusMessages: append([]jobStatusMessage(nil), p.statusMessages...),
		summaryEvents:  append([]jobSummary(nil), p.summaryEvents...),
		tables:         tables,
		mismatches:     append([]jobMismatch(nil), p.mismatches...),
		errors:         append([]string(nil), p.errors...),
	}
}

func (p jobProgressSnapshot) result() jobResult {
	return jobResult{
		StatusMessages: append([]jobStatusMessage(nil), p.statusMessages...),
		Summaries:      append([]jobSummary(nil), p.summaryEvents...),
		Mismatches:     append([]jobMismatch(nil), p.mismatches...),
		Errors:         append([]string(nil), p.errors...),
	}
}

func (p jobProgressSnapshot) sortedTables() []tableProgress {
	tables := make([]tableProgress, 0, len(p.tables))
	for _, progress := range p.tables {
		tables = append(tables, progress)
	}
	sort.Slice(tables, func(i int, j int) bool {
		if tables[i].stats.Schema != tables[j].stats.Schema {
			return tables[i].stats.Schema < tables[j].stats.Schema
		}
		return tables[i].stats.Table < tables[j].stats.Table
	})
	return tables
}

func (p tableProgress) schema() string {
	return p.stats.Schema
}

func (p tableProgress) table() string {
	return p.stats.Table
}

func (p tableProgress) sourceRows() float64 {
	return float64(p.stats.NumVerified)
}

func (p tableProgress) destinationRows() float64 {
	return float64(
		p.stats.NumSuccess +
			p.stats.NumConditionalSuccess +
			p.stats.NumMismatch +
			p.stats.NumExtraneous,
	)
}

func (p tableProgress) checkedRows() float64 {
	return float64(p.stats.NumVerified)
}

func (p tableProgress) mismatchKinds() map[string]float64 {
	return map[string]float64{
		"missing":          float64(p.stats.NumMissing),
		"mismatch":         float64(p.stats.NumMismatch),
		"column_mismatch":  float64(p.stats.NumColumnMismatch),
		"extraneous":       float64(p.stats.NumExtraneous),
		"table_definition": float64(p.tableDefinitionMismatches),
	}
}

func (p jobProgressSnapshot) errorCount() float64 {
	return float64(len(p.errors))
}
