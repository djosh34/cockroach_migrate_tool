package verifyservice

import (
	"testing"

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

	response := result.response()
	require.Len(t, response.TableSummaries, 1)
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
		response.TableSummaries[0],
	)
}
