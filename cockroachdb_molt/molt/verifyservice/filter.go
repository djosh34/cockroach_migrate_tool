package verifyservice

import (
	"regexp"

	"github.com/cockroachdb/molt/utils"
)

type JobRequest struct {
	Database      string `json:"database,omitempty"`
	IncludeSchema string `json:"include_schema,omitempty"`
	IncludeTable  string `json:"include_table,omitempty"`
	ExcludeSchema string `json:"exclude_schema,omitempty"`
	ExcludeTable  string `json:"exclude_table,omitempty"`
}

type RunRequest struct {
	Database     string
	filterConfig utils.FilterConfig
}

func (r JobRequest) Compile() (RunRequest, error) {
	filterConfig := utils.FilterConfig{
		SchemaFilter:        emptyDefaultsTo(r.IncludeSchema, utils.DefaultFilterString),
		TableFilter:         emptyDefaultsTo(r.IncludeTable, utils.DefaultFilterString),
		ExcludeSchemaFilter: r.ExcludeSchema,
		ExcludeTableFilter:  r.ExcludeTable,
	}
	if err := validateFilters(filterConfig); err != nil {
		return RunRequest{}, err
	}
	return RunRequest{Database: r.Database, filterConfig: filterConfig}, nil
}

func (r JobRequest) Validate() error {
	_, err := r.Compile()
	return err
}

func (r RunRequest) FilterConfig() utils.FilterConfig {
	return r.filterConfig
}

func (r RunRequest) ValidateSelection(config VerifyConfig) error {
	if len(config.Databases) == 0 {
		return nil
	}
	if _, err := config.ResolveDatabase(r.Database); err != nil {
		return newOperatorError(
			"request_validation",
			"invalid_database_selection",
			"request validation failed",
			operatorErrorDetail{
				Field:  "database",
				Reason: err.Error(),
			},
		)
	}
	return nil
}

func validateFilters(filterConfig utils.FilterConfig) error {
	for _, candidate := range []struct {
		field   string
		pattern string
	}{
		{field: "include_schema", pattern: filterConfig.SchemaFilter},
		{field: "include_table", pattern: filterConfig.TableFilter},
		{field: "exclude_schema", pattern: filterConfig.ExcludeSchemaFilter},
		{field: "exclude_table", pattern: filterConfig.ExcludeTableFilter},
	} {
		if candidate.pattern == "" {
			continue
		}
		if _, err := regexp.CompilePOSIX(candidate.pattern); err != nil {
			return newOperatorError(
				"request_validation",
				"invalid_filter",
				"request validation failed",
				operatorErrorDetail{
					Field:  candidate.field,
					Reason: err.Error(),
				},
			)
		}
	}
	return nil
}

func emptyDefaultsTo(value string, fallback string) string {
	if value == "" {
		return fallback
	}
	return value
}
