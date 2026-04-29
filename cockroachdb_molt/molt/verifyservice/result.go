package verifyservice

import (
	"cmp"
	"fmt"
	"slices"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/molt/utils"
	"github.com/cockroachdb/molt/verify/inconsistency"
)

type jobResult struct {
	tableSummaries map[tableKey]tableSummary
	mismatchTables map[tableKey]struct{}
	findings       []jobFinding
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

type findingView struct {
	Kind               string            `json:"kind"`
	Schema             string            `json:"schema"`
	Table              string            `json:"table"`
	PrimaryKey         map[string]string `json:"primary_key,omitempty"`
	MismatchingColumns []string          `json:"mismatching_columns,omitempty"`
	SourceValues       map[string]string `json:"source_values,omitempty"`
	DestinationValues  map[string]string `json:"destination_values,omitempty"`
	Info               []string          `json:"info,omitempty"`
	Message            string            `json:"message,omitempty"`
}

type jobFinding struct {
	kind               string
	table              tableKey
	primaryKey         map[string]string
	mismatchingColumns []string
	sourceValues       map[string]string
	destinationValues  map[string]string
	info               []string
	message            string
}

type tableIdentity struct {
	Schema string `json:"schema"`
	Table  string `json:"table"`
}

func newJobResult() jobResult {
	return jobResult{
		tableSummaries: make(map[tableKey]tableSummary),
		mismatchTables: make(map[tableKey]struct{}),
		findings:       make([]jobFinding, 0),
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
		r.findings = append(r.findings, jobFinding{
			kind:    "mismatching_table_definition",
			table:   key,
			message: report.Info,
		})
	case inconsistency.MismatchingRow:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:               "mismatching_row",
			table:              key,
			primaryKey:         renderDatumMap(report.PrimaryKeyColumns, report.PrimaryKeyValues),
			mismatchingColumns: renderNames(report.MismatchingColumns),
			sourceValues:       renderDatumMap(report.MismatchingColumns, report.TruthVals),
			destinationValues:  renderDatumMap(report.MismatchingColumns, report.TargetVals),
		})
	case inconsistency.MismatchingColumn:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:               "mismatching_column",
			table:              key,
			primaryKey:         renderDatumMap(report.PrimaryKeyColumns, report.PrimaryKeyValues),
			mismatchingColumns: renderNames(report.MismatchingColumns),
			sourceValues:       renderDatumMap(report.MismatchingColumns, report.TruthVals),
			destinationValues:  renderDatumMap(report.MismatchingColumns, report.TargetVals),
			info:               append([]string(nil), report.Info...),
		})
	case inconsistency.MissingRow:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:         "missing_row",
			table:        key,
			primaryKey:   renderDatumMap(report.PrimaryKeyColumns, report.PrimaryKeyValues),
			sourceValues: renderDatumMap(report.Columns, report.Values),
		})
	case inconsistency.ExtraneousRow:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:       "extraneous_row",
			table:      key,
			primaryKey: renderDatumMap(report.PrimaryKeyColumns, report.PrimaryKeyValues),
		})
	case utils.MissingTable:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:  "missing_table",
			table: key,
		})
	case utils.ExtraneousTable:
		key := newTableKey(string(report.Schema), string(report.Table))
		r.mismatchTables[key] = struct{}{}
		r.findings = append(r.findings, jobFinding{
			kind:  "extraneous_table",
			table: key,
		})
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

func (r jobResult) findingsView() []findingView {
	findings := make([]findingView, 0, len(r.findings))
	for _, finding := range r.findings {
		findings = append(findings, finding.view())
	}
	return findings
}

func (r jobResult) mismatchTablesView() []tableIdentity {
	keys := make([]tableKey, 0, len(r.mismatchTables))
	for key := range r.mismatchTables {
		keys = append(keys, key)
	}
	sortTableKeys(keys)

	tables := make([]tableIdentity, 0, len(keys))
	for _, key := range keys {
		tables = append(tables, tableIdentity{Schema: key.schema, Table: key.table})
	}
	return tables
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

func (f jobFinding) view() findingView {
	return findingView{
		Kind:               f.kind,
		Schema:             f.table.schema,
		Table:              f.table.table,
		PrimaryKey:         f.primaryKey,
		MismatchingColumns: f.mismatchingColumns,
		SourceValues:       f.sourceValues,
		DestinationValues:  f.destinationValues,
		Info:               f.info,
		Message:            f.message,
	}
}

func renderDatumMap(columns []tree.Name, values tree.Datums) map[string]string {
	if len(columns) == 0 || len(values) == 0 {
		return nil
	}
	rendered := make(map[string]string, min(len(columns), len(values)))
	for index, column := range columns {
		if index >= len(values) {
			break
		}
		rendered[string(column)] = renderDatum(values[index])
	}
	return rendered
}

func renderNames(names []tree.Name) []string {
	if len(names) == 0 {
		return nil
	}
	rendered := make([]string, 0, len(names))
	for _, name := range names {
		rendered = append(rendered, string(name))
	}
	return rendered
}

func renderDatum(datum tree.Datum) string {
	formatContext := tree.NewFmtCtx(tree.FmtExport | tree.FmtParsableNumerics)
	formatContext.FormatNode(datum)
	return formatContext.CloseAndGetString()
}

func statsHaveMismatch(stats inconsistency.RowStats) bool {
	return stats.NumMissing > 0 ||
		stats.NumMismatch > 0 ||
		stats.NumColumnMismatch > 0 ||
		stats.NumExtraneous > 0
}
