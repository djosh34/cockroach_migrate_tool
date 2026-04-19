package utils

import (
	"regexp"

	"github.com/cockroachdb/molt/dbtable"
)

const DefaultFilterString = ".*"

type FilterString = string

func DefaultFilterConfig() FilterConfig {
	return FilterConfig{
		SchemaFilter: DefaultFilterString,
		TableFilter:  DefaultFilterString,
	}
}

type FilterConfig struct {
	SchemaFilter        FilterString
	TableFilter         FilterString
	ExcludeSchemaFilter FilterString
	ExcludeTableFilter  FilterString
}

func FilterResult(cfg FilterConfig, r Result) (Result, error) {
	hasExclude := cfg.ExcludeSchemaFilter != "" || cfg.ExcludeTableFilter != ""
	if cfg.SchemaFilter == DefaultFilterString && cfg.TableFilter == DefaultFilterString && !hasExclude {
		return r, nil
	}
	schemaRe, err := regexp.CompilePOSIX(cfg.SchemaFilter)
	if err != nil {
		return r, err
	}
	tableRe, err := regexp.CompilePOSIX(cfg.TableFilter)
	if err != nil {
		return r, err
	}
	var excludeSchemaRe *regexp.Regexp
	var excludeTableRe *regexp.Regexp
	if hasExclude {
		excludeSchemaRe, err = regexp.CompilePOSIX(defaultFilterString(cfg.ExcludeSchemaFilter))
		if err != nil {
			return r, err
		}
		excludeTableRe, err = regexp.CompilePOSIX(defaultFilterString(cfg.ExcludeTableFilter))
		if err != nil {
			return r, err
		}
	}
	newResult := Result{
		Verified:         r.Verified[:0],
		MissingTables:    r.MissingTables[:0],
		ExtraneousTables: r.ExtraneousTables[:0],
	}
	for _, v := range r.Verified {
		if matchesFilters(v[0].Name, schemaRe, tableRe, excludeSchemaRe, excludeTableRe) {
			newResult.Verified = append(newResult.Verified, v)
		}
	}
	for _, t := range r.MissingTables {
		if matchesFilters(t.Name, schemaRe, tableRe, excludeSchemaRe, excludeTableRe) {
			newResult.MissingTables = append(newResult.MissingTables, t)
		}
	}
	for _, t := range r.ExtraneousTables {
		if matchesFilters(t.Name, schemaRe, tableRe, excludeSchemaRe, excludeTableRe) {
			newResult.ExtraneousTables = append(newResult.ExtraneousTables, t)
		}
	}
	return newResult, nil
}

func MatchesFilter(n dbtable.Name, schemaRe, tableRe *regexp.Regexp) bool {
	return schemaRe.MatchString(string(n.Schema)) && tableRe.MatchString(string(n.Table))
}

func matchesFilters(
	n dbtable.Name,
	includeSchemaRe *regexp.Regexp,
	includeTableRe *regexp.Regexp,
	excludeSchemaRe *regexp.Regexp,
	excludeTableRe *regexp.Regexp,
) bool {
	if !MatchesFilter(n, includeSchemaRe, includeTableRe) {
		return false
	}
	if excludeSchemaRe == nil || excludeTableRe == nil {
		return true
	}
	return !MatchesFilter(n, excludeSchemaRe, excludeTableRe)
}

func defaultFilterString(pattern FilterString) FilterString {
	if pattern == "" {
		return DefaultFilterString
	}
	return pattern
}

type Result struct {
	Verified [][2]dbtable.DBTable

	MissingTables    []MissingTable
	ExtraneousTables []ExtraneousTable
}

// MissingTable represents a table that is missing from a database.
type MissingTable struct {
	dbtable.DBTable
}

// ExtraneousTable represents a table that is extraneous to a database.
type ExtraneousTable struct {
	dbtable.DBTable
}

func (r *Result) AllTablesFromSource() []dbtable.DBTable {
	var res []dbtable.DBTable

	for _, vTs := range r.Verified {
		res = append(res, vTs[0])
	}

	for _, mT := range r.MissingTables {
		res = append(res, mT.DBTable)
	}

	return res
}
