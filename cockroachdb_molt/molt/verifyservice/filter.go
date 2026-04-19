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

func (r JobRequest) Validate() error {
	for _, pattern := range []string{
		r.Filters.Include.Schema,
		r.Filters.Include.Table,
		r.Filters.Exclude.Schema,
		r.Filters.Exclude.Table,
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

func (r JobRequest) FilterConfig() utils.FilterConfig {
	return utils.FilterConfig{
		SchemaFilter:        emptyDefaultsTo(r.Filters.Include.Schema, utils.DefaultFilterString),
		TableFilter:         emptyDefaultsTo(r.Filters.Include.Table, utils.DefaultFilterString),
		ExcludeSchemaFilter: r.Filters.Exclude.Schema,
		ExcludeTableFilter:  r.Filters.Exclude.Table,
	}
}

func emptyDefaultsTo(value string, fallback string) string {
	if value == "" {
		return fallback
	}
	return value
}
