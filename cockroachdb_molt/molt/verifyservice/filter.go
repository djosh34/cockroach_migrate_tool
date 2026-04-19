package verifyservice

import (
	"regexp"

	"github.com/cockroachdb/molt/utils"
)

type JobRequest struct {
	Filters JobFilters `json:"filters"`
}

type JobFilters struct {
	Include NameFilters `json:"include"`
	Exclude NameFilters `json:"exclude"`
}

type NameFilters struct {
	Schema string `json:"schema,omitempty"`
	Table  string `json:"table,omitempty"`
}

type RunRequest struct {
	filterConfig utils.FilterConfig
}

func (r JobRequest) Compile() (RunRequest, error) {
	filterConfig := utils.FilterConfig{
		SchemaFilter:        emptyDefaultsTo(r.Filters.Include.Schema, utils.DefaultFilterString),
		TableFilter:         emptyDefaultsTo(r.Filters.Include.Table, utils.DefaultFilterString),
		ExcludeSchemaFilter: r.Filters.Exclude.Schema,
		ExcludeTableFilter:  r.Filters.Exclude.Table,
	}
	if err := validateFilters(filterConfig); err != nil {
		return RunRequest{}, err
	}
	return RunRequest{filterConfig: filterConfig}, nil
}

func (r JobRequest) Validate() error {
	_, err := r.Compile()
	return err
}

func (r RunRequest) FilterConfig() utils.FilterConfig {
	return r.filterConfig
}

func validateFilters(filterConfig utils.FilterConfig) error {
	for _, pattern := range []string{
		filterConfig.SchemaFilter,
		filterConfig.TableFilter,
		filterConfig.ExcludeSchemaFilter,
		filterConfig.ExcludeTableFilter,
	} {
		if pattern == "" {
			continue
		}
		if _, err := regexp.CompilePOSIX(pattern); err != nil {
			return err
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
