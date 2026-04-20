package verifyservice

import (
	"cmp"
	"fmt"
	"slices"

	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
)

type jobResult struct {
	tableSummaries            map[tableKey]tableSummary
	mismatchTables            map[tableKey]struct{}
	tableDefinitionMismatches map[tableKey][]string
}

type tableKey struct {
	schema string
	table  string
}

type tableSummary struct {
	Schema            string `json:"schema"`
	Table             string `json:"table"`
	NumVerified       int    `json:"num_verified"`
	NumSuccess        int    `json:"num_success"`
	NumMissing        int    `json:"num_missing"`
	NumMismatch       int    `json:"num_mismatch"`
	NumColumnMismatch int    `json:"num_column_mismatch"`
	NumExtraneous     int    `json:"num_extraneous"`
	NumLiveRetry      int    `json:"num_live_retry"`
}

type jobResultView struct {
	TableSummaries            []tableSummary                `json:"table_summaries"`
	MismatchTables            []tableIdentityView           `json:"mismatch_tables"`
	TableDefinitionMismatches []tableDefinitionMismatchView `json:"table_definition_mismatches"`
}

type tableIdentityView struct {
	Schema string `json:"schema"`
	Table  string `json:"table"`
}

type tableDefinitionMismatchView struct {
	Schema  string `json:"schema"`
	Table   string `json:"table"`
	Message string `json:"message"`
}

func newJobResult() jobResult {
	return jobResult{
		tableSummaries:            make(map[tableKey]tableSummary),
		mismatchTables:            make(map[tableKey]struct{}),
		tableDefinitionMismatches: make(map[tableKey][]string),
	}
}

func (r *jobResult) recordReport(obj any) {
	switch report := obj.(type) {
	case inconsistency.SummaryReport:
		key := newTableKey(report.Stats.Schema, report.Stats.Table)
		r.tableSummaries[key] = r.tableSummaries[key].accumulate(tableSummary{
			Schema:            report.Stats.Schema,
			Table:             report.Stats.Table,
			NumVerified:       report.Stats.NumVerified,
			NumSuccess:        report.Stats.NumSuccess,
			NumMissing:        report.Stats.NumMissing,
			NumMismatch:       report.Stats.NumMismatch,
			NumColumnMismatch: report.Stats.NumColumnMismatch,
			NumExtraneous:     report.Stats.NumExtraneous,
			NumLiveRetry:      report.Stats.NumLiveRetry,
		})
		if statsHaveMismatch(report.Stats) {
			r.mismatchTables[key] = struct{}{}
		}
	case inconsistency.MismatchingTableDefinition:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.tableDefinitionMismatches[key] = append(r.tableDefinitionMismatches[key], report.Info)
	case inconsistency.MismatchingRow:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	case inconsistency.MismatchingColumn:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	case inconsistency.MissingRow:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	case inconsistency.ExtraneousRow:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	case utils.MissingTable:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	case utils.ExtraneousTable:
		r.mismatchTables[newTableKey(string(report.Schema), string(report.Table))] = struct{}{}
	}
}

func (s tableSummary) accumulate(other tableSummary) tableSummary {
	return tableSummary{
		Schema:            other.Schema,
		Table:             other.Table,
		NumVerified:       s.NumVerified + other.NumVerified,
		NumSuccess:        s.NumSuccess + other.NumSuccess,
		NumMissing:        s.NumMissing + other.NumMissing,
		NumMismatch:       s.NumMismatch + other.NumMismatch,
		NumColumnMismatch: s.NumColumnMismatch + other.NumColumnMismatch,
		NumExtraneous:     s.NumExtraneous + other.NumExtraneous,
		NumLiveRetry:      s.NumLiveRetry + other.NumLiveRetry,
	}
}

func (r jobResult) hasData() bool {
	return len(r.tableSummaries) > 0 ||
		len(r.mismatchTables) > 0 ||
		len(r.tableDefinitionMismatches) > 0
}

func (r jobResult) hasMismatch() bool {
	return len(r.mismatchTables) > 0 || len(r.tableDefinitionMismatches) > 0
}

func (r jobResult) mismatchFailure() *operatorError {
	mismatchTables := r.mismatchTablesView()
	if len(mismatchTables) == 0 {
		return nil
	}
	details := make([]operatorErrorDetail, 0, len(mismatchTables))
	for _, table := range mismatchTables {
		details = append(details, operatorErrorDetail{
			Reason: fmt.Sprintf("mismatch detected for %s.%s", table.Schema, table.Table),
		})
	}
	return newOperatorError(
		"mismatch",
		"mismatch_detected",
		fmt.Sprintf("verify detected mismatches in %d table", len(mismatchTables)),
		details...,
	)
}

func (r jobResult) response() *jobResultView {
	return &jobResultView{
		TableSummaries:            r.tableSummariesView(),
		MismatchTables:            r.mismatchTablesView(),
		TableDefinitionMismatches: r.tableDefinitionMismatchesView(),
	}
}

func (r jobResult) tableSummariesView() []tableSummary {
	keys := make([]tableKey, 0, len(r.tableSummaries))
	for key := range r.tableSummaries {
		keys = append(keys, key)
	}
	sortTableKeys(keys)

	summaries := make([]tableSummary, 0, len(keys))
	for _, key := range keys {
		summaries = append(summaries, r.tableSummaries[key])
	}
	return summaries
}

func (r jobResult) mismatchTablesView() []tableIdentityView {
	keys := make([]tableKey, 0, len(r.mismatchTables))
	for key := range r.mismatchTables {
		keys = append(keys, key)
	}
	sortTableKeys(keys)

	tables := make([]tableIdentityView, 0, len(keys))
	for _, key := range keys {
		tables = append(tables, tableIdentityView{Schema: key.schema, Table: key.table})
	}
	return tables
}

func (r jobResult) tableDefinitionMismatchesView() []tableDefinitionMismatchView {
	keys := make([]tableKey, 0, len(r.tableDefinitionMismatches))
	for key := range r.tableDefinitionMismatches {
		keys = append(keys, key)
	}
	sortTableKeys(keys)

	mismatches := make([]tableDefinitionMismatchView, 0)
	for _, key := range keys {
		for _, message := range r.tableDefinitionMismatches[key] {
			mismatches = append(mismatches, tableDefinitionMismatchView{
				Schema:  key.schema,
				Table:   key.table,
				Message: message,
			})
		}
	}
	return mismatches
}

func sortTableKeys(keys []tableKey) {
	slices.SortFunc(keys, func(left tableKey, right tableKey) int {
		if bySchema := cmp.Compare(left.schema, right.schema); bySchema != 0 {
			return bySchema
		}
		return cmp.Compare(left.table, right.table)
	})
}

func newTableKey(schema string, table string) tableKey {
	return tableKey{schema: schema, table: table}
}

func statsHaveMismatch(stats inconsistency.RowStats) bool {
	return stats.NumMissing > 0 ||
		stats.NumMismatch > 0 ||
		stats.NumColumnMismatch > 0 ||
		stats.NumExtraneous > 0
}
