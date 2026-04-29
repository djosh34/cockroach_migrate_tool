package verifyservice

import (
	"testing"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/verify/inconsistency"
	"github.com/stretchr/testify/require"
)

func TestJobResultAccumulatesShardSummariesPerTable(t *testing.T) {
	t.Parallel()

	result := newJobResult()
	result.recordReport(inconsistency.SummaryReport{
		Stats: inconsistency.RowStats{
			Schema:            "public",
			Table:             "parents",
			NumVerified:       2,
			NumSuccess:        2,
			NumMismatch:       0,
			NumColumnMismatch: 0,
			NumExtraneous:     0,
			NumLiveRetry:      0,
		},
	})
	result.recordReport(inconsistency.SummaryReport{
		Stats: inconsistency.RowStats{
			Schema:            "public",
			Table:             "parents",
			NumVerified:       0,
			NumSuccess:        0,
			NumMismatch:       0,
			NumColumnMismatch: 0,
			NumExtraneous:     0,
			NumLiveRetry:      0,
		},
	})

	require.Len(t, result.tableSummaries, 1)
	require.Equal(
		t,
		tableSummary{
			Schema:            "public",
			Table:             "parents",
			NumVerified:       2,
			NumSuccess:        2,
			NumMissing:        0,
			NumMismatch:       0,
			NumColumnMismatch: 0,
			NumExtraneous:     0,
			NumLiveRetry:      0,
		},
		result.tableSummaries[newTableKey("public", "parents")],
	)
}

func TestJobResultProjectsStructuredMismatchFindings(t *testing.T) {
	t.Parallel()

	result := newJobResult()
	result.recordReport(inconsistency.SummaryReport{
		Stats: inconsistency.RowStats{
			Schema:            "public",
			Table:             "accounts",
			NumVerified:       7,
			NumSuccess:        6,
			NumColumnMismatch: 1,
		},
	})
	result.recordReport(inconsistency.MismatchingColumn{
		Name: dbtable.Name{
			Schema: "public",
			Table:  "accounts",
		},
		PrimaryKeyColumns: []tree.Name{"id"},
		PrimaryKeyValues:  tree.Datums{tree.NewDInt(101)},
		MismatchingColumns: []tree.Name{
			"balance",
		},
		TruthVals:  tree.Datums{tree.NewDInt(17)},
		TargetVals: tree.Datums{tree.NewDInt(23)},
		Info:       []string{"balance mismatch"},
	})

	require.Equal(
		t,
		[]findingView{
			{
				Kind:   "mismatching_column",
				Schema: "public",
				Table:  "accounts",
				PrimaryKey: map[string]string{
					"id": "101",
				},
				MismatchingColumns: []string{"balance"},
				SourceValues: map[string]string{
					"balance": "17",
				},
				DestinationValues: map[string]string{
					"balance": "23",
				},
				Info: []string{"balance mismatch"},
			},
		},
		result.findingsView(),
	)
	require.Equal(
		t,
		&operatorError{
			category: "mismatch",
			code:     "mismatch_detected",
			message:  "verify detected mismatches in 1 table",
			details: []operatorErrorDetail{
				{Reason: "mismatch detected for public.accounts"},
			},
		},
		result.mismatchFailure(),
	)
}
