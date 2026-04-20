package verifyservice

import (
	"testing"

	"github.com/cockroachdb/molt/utils"
	"github.com/stretchr/testify/require"
)

func TestJobRequestCompileBuildsTypedRunRequest(t *testing.T) {
	t.Parallel()

	runRequest, err := (JobRequest{
		IncludeSchema: "^public$",
		IncludeTable:  "accounts;$(touch /tmp/pwned)|orders",
		ExcludeSchema: "audit|tmp;rm -rf /",
		ExcludeTable:  "^tmp_",
	}).Compile()
	require.NoError(t, err)
	require.Equal(t, utils.FilterConfig{
		SchemaFilter:        "^public$",
		TableFilter:         "accounts;$(touch /tmp/pwned)|orders",
		ExcludeSchemaFilter: "audit|tmp;rm -rf /",
		ExcludeTableFilter:  "^tmp_",
	}, runRequest.FilterConfig())
}

func TestJobRequestCompileRejectsInvalidRegex(t *testing.T) {
	t.Parallel()

	_, err := (JobRequest{
		IncludeSchema: "[",
	}).Compile()
	require.Error(t, err)
}
