package verifyservice

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestJobRequestCompileNormalizesFlexibleDatabaseSelectors(t *testing.T) {
	t.Parallel()

	normalized, err := (JobRequest{
		DefaultSchemaMatch: matchExpression{"public"},
		DefaultTableMatch:  matchExpression{"*"},
		Databases: databaseRequestValue{
			{DatabaseMatch: "app"},
			{
				DatabaseMatch: "billing",
				TableMatch:    matchExpression{"invoices", "payments"},
			},
		},
	}).Compile()
	require.NoError(t, err)
	require.Equal(t, NormalizedJobRequest{
		DatabaseSelections: []NormalizedDatabaseSelection{
			{
				DatabaseMatch: "app",
				SchemaGlobs:   []string{"public"},
				TableGlobs:    []string{"*"},
			},
			{
				DatabaseMatch: "billing",
				SchemaGlobs:   []string{"public"},
				TableGlobs:    []string{"invoices", "payments"},
			},
		},
	}, normalized)
}

func TestJobRequestCompileRejectsInvalidGlob(t *testing.T) {
	t.Parallel()

	_, err := (JobRequest{
		DefaultSchemaMatch: matchExpression{"["},
	}).Compile()
	require.Equal(
		t,
		&operatorError{
			category: "request_validation",
			code:     "invalid_glob",
			message:  "request validation failed",
			details: []operatorErrorDetail{
				{
					Field:  "default_schema_match",
					Reason: "syntax error in pattern",
				},
			},
		},
		err,
	)
}
