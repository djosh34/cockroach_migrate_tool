package verifyservice

import (
	"encoding/json"
	"fmt"
	"path"
	"regexp"
	"slices"
	"strings"

	"github.com/cockroachdb/molt/utils"
)

type JobRequest struct {
	DefaultSchemaMatch matchExpression      `json:"default_schema_match,omitempty"`
	DefaultTableMatch  matchExpression      `json:"default_table_match,omitempty"`
	Databases          databaseRequestValue `json:"databases,omitempty"`
}

type matchExpression []string

type jobDatabaseRequest struct {
	DatabaseMatch string          `json:"database_match"`
	SchemaMatch   matchExpression `json:"schema_match,omitempty"`
	TableMatch    matchExpression `json:"table_match,omitempty"`
}

type databaseRequestValue []jobDatabaseRequest

type NormalizedJobRequest struct {
	DatabaseSelections []NormalizedDatabaseSelection
}

type NormalizedDatabaseSelection struct {
	DatabaseMatch string
	SchemaGlobs   []string
	TableGlobs    []string
}

type ResolvedJobPlan struct {
	Databases []ResolvedDatabasePlan
}

type ResolvedDatabasePlan struct {
	Database       ResolvedDatabasePair
	SchemaGlobs    []string
	TableGlobs     []string
	InitialSchemas []string
}

type RunRequest struct {
	DatabaseName     string
	ResolvedDatabase ResolvedDatabasePair
	filterConfig     utils.FilterConfig
}

func (e *matchExpression) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		*e = nil
		return nil
	}

	var single string
	if err := json.Unmarshal(data, &single); err == nil {
		*e = matchExpression{single}
		return nil
	}

	var multiple []string
	if err := json.Unmarshal(data, &multiple); err == nil {
		*e = matchExpression(multiple)
		return nil
	}

	return fmt.Errorf("must be a string or array of strings")
}

func (v *databaseRequestValue) UnmarshalJSON(data []byte) error {
	if string(data) == "null" {
		*v = nil
		return nil
	}

	var singleString string
	if err := json.Unmarshal(data, &singleString); err == nil {
		*v = databaseRequestValue{{DatabaseMatch: singleString}}
		return nil
	}

	var singleObject jobDatabaseRequest
	if err := json.Unmarshal(data, &singleObject); err == nil {
		*v = databaseRequestValue{singleObject}
		return nil
	}

	var rawItems []json.RawMessage
	if err := json.Unmarshal(data, &rawItems); err != nil {
		return fmt.Errorf("must be a string, object, or array")
	}

	items := make(databaseRequestValue, 0, len(rawItems))
	for _, rawItem := range rawItems {
		var itemString string
		if err := json.Unmarshal(rawItem, &itemString); err == nil {
			items = append(items, jobDatabaseRequest{DatabaseMatch: itemString})
			continue
		}

		var itemObject jobDatabaseRequest
		if err := json.Unmarshal(rawItem, &itemObject); err == nil {
			items = append(items, itemObject)
			continue
		}

		return fmt.Errorf("array items must be strings or objects")
	}

	*v = items
	return nil
}

func (r JobRequest) Compile() (NormalizedJobRequest, error) {
	defaultSchemaGlobs := defaultGlobs([]string(r.DefaultSchemaMatch))
	defaultTableGlobs := defaultGlobs([]string(r.DefaultTableMatch))
	if err := validateGlobList("default_schema_match", defaultSchemaGlobs); err != nil {
		return NormalizedJobRequest{}, err
	}
	if err := validateGlobList("default_table_match", defaultTableGlobs); err != nil {
		return NormalizedJobRequest{}, err
	}

	databaseRequests := slices.Clone([]jobDatabaseRequest(r.Databases))
	if len(databaseRequests) == 0 {
		databaseRequests = []jobDatabaseRequest{{DatabaseMatch: "*"}}
	}

	selections := make([]NormalizedDatabaseSelection, 0, len(databaseRequests))
	for _, databaseRequest := range databaseRequests {
		if databaseRequest.DatabaseMatch == "" {
			return NormalizedJobRequest{}, newOperatorError(
				"request_validation",
				"invalid_database_selection",
				"request validation failed",
				operatorErrorDetail{
					Field:  "databases.database_match",
					Reason: "database_match must be set",
				},
			)
		}
		if err := validateGlobList("databases.database_match", []string{databaseRequest.DatabaseMatch}); err != nil {
			return NormalizedJobRequest{}, err
		}

		schemaGlobs := defaultSchemaGlobs
		if len(databaseRequest.SchemaMatch) > 0 {
			schemaGlobs = []string(databaseRequest.SchemaMatch)
		}
		tableGlobs := defaultTableGlobs
		if len(databaseRequest.TableMatch) > 0 {
			tableGlobs = []string(databaseRequest.TableMatch)
		}
		if err := validateGlobList("schema_match", schemaGlobs); err != nil {
			return NormalizedJobRequest{}, err
		}
		if err := validateGlobList("table_match", tableGlobs); err != nil {
			return NormalizedJobRequest{}, err
		}

		selections = append(selections, NormalizedDatabaseSelection{
			DatabaseMatch: databaseRequest.DatabaseMatch,
			SchemaGlobs:   slices.Clone(schemaGlobs),
			TableGlobs:    slices.Clone(tableGlobs),
		})
	}

	return NormalizedJobRequest{DatabaseSelections: selections}, nil
}

func (r NormalizedJobRequest) Resolve(config VerifyConfig) (ResolvedJobPlan, error) {
	resolvedPairs, err := config.ResolveAllDatabases()
	if err != nil {
		return ResolvedJobPlan{}, err
	}

	plans := make([]ResolvedDatabasePlan, 0, len(resolvedPairs))
	for _, pair := range resolvedPairs {
		var schemaGlobs []string
		var tableGlobs []string
		for _, selection := range r.DatabaseSelections {
			matched, err := path.Match(selection.DatabaseMatch, pair.Name)
			if err != nil {
				return ResolvedJobPlan{}, newOperatorError(
					"request_validation",
					"invalid_glob",
					"request validation failed",
					operatorErrorDetail{
						Field:  "databases.database_match",
						Reason: err.Error(),
					},
				)
			}
			if !matched {
				continue
			}
			schemaGlobs = appendUniqueStrings(schemaGlobs, selection.SchemaGlobs...)
			tableGlobs = appendUniqueStrings(tableGlobs, selection.TableGlobs...)
		}
		if len(schemaGlobs) == 0 && len(tableGlobs) == 0 {
			continue
		}

		plan := ResolvedDatabasePlan{
			Database:    pair,
			SchemaGlobs: schemaGlobs,
			TableGlobs:  tableGlobs,
		}
		if initialSchemas, ok := literalPatternsOnly(schemaGlobs); ok {
			plan.InitialSchemas = initialSchemas
		}
		plans = append(plans, plan)
	}

	if len(plans) == 0 {
		return ResolvedJobPlan{}, newOperatorError(
			"request_validation",
			"invalid_database_selection",
			"request validation failed",
			operatorErrorDetail{
				Field:  "databases",
				Reason: "no configured databases matched the request",
			},
		)
	}

	return ResolvedJobPlan{Databases: plans}, nil
}

func (r RunRequest) FilterConfig() utils.FilterConfig {
	return r.filterConfig
}

func (p ResolvedDatabasePlan) RunRequest() (RunRequest, error) {
	filterConfig, err := filterConfigFromGlobs(p.SchemaGlobs, p.TableGlobs)
	if err != nil {
		return RunRequest{}, err
	}
	return RunRequest{
		DatabaseName:     p.Database.Name,
		ResolvedDatabase: p.Database,
		filterConfig:     filterConfig,
	}, nil
}

func defaultGlobs(globs []string) []string {
	if len(globs) == 0 {
		return []string{"*"}
	}
	return slices.Clone(globs)
}

func validateGlobList(field string, globs []string) error {
	for _, glob := range globs {
		if _, err := path.Match(glob, "validate"); err != nil {
			return newOperatorError(
				"request_validation",
				"invalid_glob",
				"request validation failed",
				operatorErrorDetail{
					Field:  field,
					Reason: err.Error(),
				},
			)
		}
	}
	return nil
}

func literalPatternsOnly(globs []string) ([]string, bool) {
	literals := make([]string, 0, len(globs))
	for _, glob := range globs {
		if strings.ContainsAny(glob, "*?[") {
			return nil, false
		}
		literals = append(literals, glob)
	}
	return appendUniqueStrings(nil, literals...), true
}

func appendUniqueStrings(existing []string, candidates ...string) []string {
	for _, candidate := range candidates {
		if slices.Contains(existing, candidate) {
			continue
		}
		existing = append(existing, candidate)
	}
	return existing
}

func filterConfigFromGlobs(schemaGlobs []string, tableGlobs []string) (utils.FilterConfig, error) {
	schemaRegex, err := globsToRegex(schemaGlobs)
	if err != nil {
		return utils.FilterConfig{}, err
	}
	tableRegex, err := globsToRegex(tableGlobs)
	if err != nil {
		return utils.FilterConfig{}, err
	}
	return utils.FilterConfig{
		SchemaFilter: schemaRegex,
		TableFilter:  tableRegex,
	}, nil
}

func globsToRegex(globs []string) (string, error) {
	if len(globs) == 0 {
		return utils.DefaultFilterString, nil
	}

	regexParts := make([]string, 0, len(globs))
	for _, glob := range globs {
		regexPart, err := globToRegex(glob)
		if err != nil {
			return "", err
		}
		regexParts = append(regexParts, regexPart)
	}
	if len(regexParts) == 1 && regexParts[0] == ".*" {
		return utils.DefaultFilterString, nil
	}
	return "^(?:" + strings.Join(regexParts, "|") + ")$", nil
}

func globToRegex(glob string) (string, error) {
	var builder strings.Builder
	for index := 0; index < len(glob); index++ {
		switch glob[index] {
		case '*':
			builder.WriteString(".*")
		case '?':
			builder.WriteByte('.')
		case '[':
			closing := strings.IndexByte(glob[index+1:], ']')
			if closing < 0 {
				return "", path.ErrBadPattern
			}

			class := glob[index+1 : index+1+closing]
			if class == "" {
				return "", path.ErrBadPattern
			}
			builder.WriteByte('[')
			if class[0] == '!' {
				builder.WriteByte('^')
				class = class[1:]
			}
			if class == "" {
				return "", path.ErrBadPattern
			}
			builder.WriteString(class)
			builder.WriteByte(']')
			index += closing + 1
		default:
			builder.WriteString(regexp.QuoteMeta(string(glob[index])))
		}
	}
	return builder.String(), nil
}
