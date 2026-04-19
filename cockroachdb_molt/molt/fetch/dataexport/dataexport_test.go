package dataexport

import (
	"testing"

	"github.com/stretchr/testify/require"
)

func TestParseFlagStrings(t *testing.T) {

	for _, tc := range []struct {
		desc     string
		input    string
		expected []string
	}{
		{
			desc:     "empty string",
			input:    "",
			expected: []string(nil),
		},
		{
			desc:     "single dash flag",
			input:    "-v",
			expected: []string{"-v"},
		},
		{
			desc:     "flag without arg",
			input:    "--bool-only",
			expected: []string{"--bool-only"},
		},
		{
			desc:     "flag with arg",
			input:    "--source 'the source'",
			expected: []string{"--source", "'the source'"},
		},
		{
			desc:     "flag with string arg",
			input:    "--source \"the source\"",
			expected: []string{"--source", "\"the source\""},
		},
		{
			desc:     "flag with mix of string arg and non string arg",
			input:    "--source \"the source\" --target target",
			expected: []string{"--source", "\"the source\"", "--target", "target"},
		},
		{
			desc:     "flag with mix of string and non string arg and no arg",
			input:    "--source \"the source\" --finish --target target --validate",
			expected: []string{"--source", "\"the source\"", "--finish", "--target", "target", "--validate"},
		},
	} {
		t.Run(tc.desc, func(t *testing.T) {
			output := parseFlagStrings(tc.input)
			require.Equal(t, tc.expected, output)
		})
	}
}

func TestBuildMapFromSlice(t *testing.T) {

	for _, tc := range []struct {
		desc        string
		input       []string
		expected    map[string]string
		expectError bool
	}{
		{
			desc:     "empty string",
			input:    []string{},
			expected: map[string]string{},
		},
		{
			desc:     "single flag without arg",
			input:    []string{"--noarg"},
			expected: map[string]string{"--noarg": ""},
		},
		{
			desc:     "single flag with arg",
			input:    []string{"--single", "the arg"},
			expected: map[string]string{"--single": "the arg"},
		},
		{
			desc:     "single flag with quoted arg",
			input:    []string{"--single", "\"the arg\""},
			expected: map[string]string{"--single": "\"the arg\""},
		},
		{
			desc:     "mixed arguments",
			input:    []string{"--single", "\"the arg\"", "--bool-only", "--second", "new"},
			expected: map[string]string{"--single": "\"the arg\"", "--bool-only": "", "--second": "new"},
		},
		{
			desc:     "end with two flags with no args",
			input:    []string{"--single", "\"the arg\"", "--bool-only", "-int-only"},
			expected: map[string]string{"--single": "\"the arg\"", "--bool-only": "", "-int-only": ""},
		},
		{
			desc:        "error out on invalid args in middle",
			input:       []string{"--single", "\"the arg\"", "invalid", "--bool-only", "--second", "new"},
			expected:    map[string]string{},
			expectError: true,
		},
		{
			desc:        "error out on invalid args at end",
			input:       []string{"--single", "\"the arg\"", "--bool-only", "--second", "new", "invalid"},
			expected:    map[string]string{},
			expectError: true,
		},
	} {
		t.Run(tc.desc, func(t *testing.T) {
			output, err := buildFlagMapFromSlice(tc.input)
			require.Equal(t, tc.expected, output)
			if tc.expectError {
				require.Error(t, err)
			} else {
				require.NoError(t, err)
			}
		})
	}
}

func TestHandleFlagOverrides(t *testing.T) {

	for _, tc := range []struct {
		desc          string
		defaultFlags  map[string]string
		overrideFlags map[string]string
		expected      []string
	}{
		{
			desc:          "empty set of flags",
			defaultFlags:  map[string]string{},
			overrideFlags: map[string]string{},
			expected:      []string{},
		},
		{
			desc:          "default flags only",
			defaultFlags:  map[string]string{"--key": "value", "--new-key": "value2"},
			overrideFlags: map[string]string{},
			expected:      []string{"--key", "value", "--new-key", "value2"},
		},
		{
			desc:          "default and override flags don't intersect",
			defaultFlags:  map[string]string{"--key": "value", "--new-key": "value2"},
			overrideFlags: map[string]string{"--a-flag": "value3"},
			expected:      []string{"--a-flag", "value3", "--key", "value", "--new-key", "value2"},
		},
		{
			desc:          "override flag overrides default",
			defaultFlags:  map[string]string{"--key": "value", "--new-key": "value2"},
			overrideFlags: map[string]string{"--a-flag": "value3", "--new-key": "value4"},
			expected:      []string{"--a-flag", "value3", "--key", "value", "--new-key", "value4"},
		},
		{
			desc:          "keys are arranged in sorted order",
			defaultFlags:  map[string]string{"--zoo": "value", "--moo": "value2"},
			overrideFlags: map[string]string{"--ahh": "value3", "--noo": "value4", "--voo": ""},
			expected:      []string{"--ahh", "value3", "--moo", "value2", "--noo", "value4", "--voo", "--zoo", "value"},
		},
	} {
		t.Run(tc.desc, func(t *testing.T) {
			output := handleFlagOverrides(tc.defaultFlags, tc.overrideFlags)
			require.Equal(t, tc.expected, output)
		})
	}
}

// Only need to test the success and error cases since the various
// sub-edge cases are caught in the tests of the helper methods.
func TestGetFlagList(t *testing.T) {
	flags, err := getFlagList(DefaultReplicatorFlags, "--parallelism 256")
	require.NoError(t, err)
	require.Equal(t, []string{"--parallelism", "256", "-v"}, flags)

	flags, err = getFlagList(DefaultReplicatorFlags, "badflag")
	require.EqualError(t, err, "invalid flag 'badflag'")
	require.Equal(t, []string(nil), flags)
}
