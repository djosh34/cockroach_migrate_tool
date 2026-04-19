package scaletesting

import (
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestExtractVerifyResult(t *testing.T) {
	content, err := os.ReadFile("./testdata/finished-verify.txt")
	require.NoError(t, err)

	verResults, err := extractVerifyResults(string(content))
	require.NoError(t, err)
	require.Len(t, verResults, 4)

	for _, result := range verResults {
		require.Equal(t, result.numSuccessRows, result.numTruthRows)
		require.NotEqual(t, "", result.originalLog)
	}
}

func TestDidVerifyPass(t *testing.T) {
	successfulResults := []*verifyResult{{
		numTruthRows:   1000,
		numSuccessRows: 1000,
		originalLog:    "log1",
	}, {
		numTruthRows:   12345,
		numSuccessRows: 12345,
		originalLog:    "log2",
	}}
	failedResults := []*verifyResult{{
		numTruthRows:   1001,
		numSuccessRows: 1000,
		originalLog:    "log3",
	}, {
		numTruthRows:   12344,
		numSuccessRows: 12345,
		originalLog:    "log4",
	}}

	require.True(t, didVerifyPass(successfulResults))
	require.False(t, didVerifyPass(failedResults))
}
