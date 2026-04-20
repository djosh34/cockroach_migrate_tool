package verifyservice

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestNormalizeRawTableValueFailsLoudlyForUnsupportedValues(t *testing.T) {
	t.Parallel()

	value, err := normalizeRawTableValue("payload", func() {})
	require.Nil(t, value)
	require.ErrorContains(t, err, "column payload is not JSON representable")
}
