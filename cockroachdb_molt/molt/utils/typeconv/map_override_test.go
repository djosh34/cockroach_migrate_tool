package typeconv

import (
	"testing"

	_ "github.com/cockroachdb/cockroachdb-parser/pkg/sql/types"
	"github.com/cockroachdb/datadriven"
	"github.com/rs/zerolog"
	"github.com/stretchr/testify/require"
)

func TestMapOverrideDatadriven(t *testing.T) {
	datadriven.Walk(t, "./testdata", func(t *testing.T, path string) {
		datadriven.RunTest(t, path, func(t *testing.T, d *datadriven.TestData) string {
			jsonInput := []byte(d.Input)
			require.Equal(t, "type-mapping", d.Cmd)
			expectErr := false
			for _, cmd := range d.CmdArgs {
				switch cmd.Key {
				case "expect-error":
					expectErr = true
				}
			}
			res, err := getOverrideTypeMapFromJsonBytes(jsonInput, zerolog.Nop())
			if expectErr {
				return err.Error()
			}
			require.NoError(t, err)
			return res.String()
		})
	})
}
