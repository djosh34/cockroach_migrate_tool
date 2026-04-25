package verifyservice_test

import (
	"context"
	"strings"
	"testing"

	"github.com/getkin/kin-openapi/openapi3"
	"github.com/stretchr/testify/require"
)

func TestVerifyServiceOpenAPIContractExistsAndValidates(t *testing.T) {
	t.Parallel()

	doc := loadVerifyServiceOpenAPIDoc(t)
	require.Equal(t, "3.0.3", doc.OpenAPI)
	require.NoError(t, doc.Validate(context.Background()))

	require.Len(t, doc.Servers, 1)
	require.Equal(t, "http://localhost:8080", doc.Servers[0].URL)
	require.Contains(t, doc.Servers[0].Description, "listener.bind_addr")

	require.True(
		t,
		strings.Contains(doc.Info.Description, "listener.bind_addr") ||
			strings.Contains(doc.Servers[0].Description, "listener.bind_addr"),
		"spec should explain that the actual host and port come from listener.bind_addr",
	)
}

func TestVerifyServiceOpenAPIJobEndpointsContract(t *testing.T) {
	t.Parallel()

	doc := loadVerifyServiceOpenAPIDoc(t)

	postJobs := doc.Paths.Find("/jobs").Post
	require.NotNil(t, postJobs, "spec should document POST /jobs")

	jobRequest := requestJSONSchema(t, postJobs)
	require.True(t, jobRequest.Type.Is(openapi3.TypeObject))
	requireNoAdditionalProperties(t, jobRequest)
	require.Empty(t, jobRequest.Required)
	require.Len(t, jobRequest.Properties, 4)
	for _, propertyName := range []string{
		"include_schema",
		"include_table",
		"exclude_schema",
		"exclude_table",
	} {
		require.True(
			t,
			requirePropertySchema(t, jobRequest, propertyName).Type.Is(openapi3.TypeString),
			"%s should be documented as a string filter field",
			propertyName,
		)
	}

	jobAccepted := responseJSONSchema(t, postJobs, 202)
	require.True(t, jobAccepted.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(t, []string{"job_id", "status"}, jobAccepted.Required)
	require.True(t, requirePropertySchema(t, jobAccepted, "job_id").Type.Is(openapi3.TypeString))
	requireEnumStrings(t, requirePropertySchema(t, jobAccepted, "status"), "running")
	requireResponseExample(t, postJobs, 202, "accepted")

	for _, statusCode := range []int{400, 409, 413} {
		operatorErrorEnvelope := responseJSONSchema(t, postJobs, statusCode)
		requireStructuredOperatorErrorEnvelope(t, operatorErrorEnvelope)
	}
	requireResponseExample(t, postJobs, 400, "unknown_field")
	requireResponseExample(t, postJobs, 400, "multiple_documents")
	requireResponseExample(t, postJobs, 400, "invalid_filter")
	requireResponseExample(t, postJobs, 409, "job_already_running")
	requireResponseExample(t, postJobs, 413, "request_body_too_large")

	getJob := doc.Paths.Find("/jobs/{job_id}").Get
	require.NotNil(t, getJob, "spec should document GET /jobs/{job_id}")
	require.Len(t, getJob.Parameters, 1)
	require.Equal(t, "job_id", getJob.Parameters[0].Value.Name)
	require.Equal(t, "path", getJob.Parameters[0].Value.In)
	require.True(t, getJob.Parameters[0].Value.Required)
	require.True(t, getJob.Parameters[0].Value.Schema.Value.Type.Is(openapi3.TypeString))

	jobResponse := responseJSONSchema(t, getJob, 200)
	require.True(t, jobResponse.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(t, []string{"job_id", "status"}, jobResponse.Required)
	require.True(t, requirePropertySchema(t, jobResponse, "job_id").Type.Is(openapi3.TypeString))
	requireEnumStrings(
		t,
		requirePropertySchema(t, jobResponse, "status"),
		"running",
		"succeeded",
		"failed",
		"stopped",
	)
	for _, exampleName := range []string{"running", "succeeded", "failed", "stopped"} {
		requireResponseExample(t, getJob, 200, exampleName)
	}
	requireStructuredOperatorErrorEnvelope(t, responseJSONSchema(t, getJob, 404))
	requireResponseExample(t, getJob, 404, "job_not_found")

	stopJob := doc.Paths.Find("/jobs/{job_id}/stop").Post
	require.NotNil(t, stopJob, "spec should document POST /jobs/{job_id}/stop")
	require.Len(t, stopJob.Parameters, 1)
	require.Equal(t, "job_id", stopJob.Parameters[0].Value.Name)
	stopRequest := requestJSONSchema(t, stopJob)
	require.True(t, stopRequest.Type.Is(openapi3.TypeObject))
	require.Empty(t, stopRequest.Properties)
	require.Empty(t, stopRequest.Required)
	requireNoAdditionalProperties(t, stopRequest)

	stopResponse := responseJSONSchema(t, stopJob, 200)
	require.True(t, stopResponse.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(t, []string{"job_id", "status"}, stopResponse.Required)
	requireEnumStrings(t, requirePropertySchema(t, stopResponse, "status"), "stopping")
	requireResponseExample(t, stopJob, 200, "stopping")
	requireStructuredOperatorErrorEnvelope(t, responseJSONSchema(t, stopJob, 404))
	requireResponseExample(t, stopJob, 404, "job_not_found")
}

func TestVerifyServiceOpenAPIRawTableAndMetricsContract(t *testing.T) {
	t.Parallel()

	doc := loadVerifyServiceOpenAPIDoc(t)

	rawTables := doc.Paths.Find("/tables/raw").Post
	require.NotNil(t, rawTables, "spec should document POST /tables/raw")

	rawTableRequest := requestJSONSchema(t, rawTables)
	require.True(t, rawTableRequest.Type.Is(openapi3.TypeObject))
	requireNoAdditionalProperties(t, rawTableRequest)
	require.ElementsMatch(t, []string{"database", "schema", "table"}, rawTableRequest.Required)
	requireEnumStrings(
		t,
		requirePropertySchema(t, rawTableRequest, "database"),
		"source",
		"destination",
	)
	require.True(t, requirePropertySchema(t, rawTableRequest, "schema").Type.Is(openapi3.TypeString))
	require.True(t, requirePropertySchema(t, rawTableRequest, "table").Type.Is(openapi3.TypeString))

	rawTableResponse := responseJSONSchema(t, rawTables, 200)
	require.True(t, rawTableResponse.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(
		t,
		[]string{"database", "schema", "table", "columns", "rows"},
		rawTableResponse.Required,
	)
	requireEnumStrings(
		t,
		requirePropertySchema(t, rawTableResponse, "database"),
		"source",
		"destination",
	)
	require.True(t, requirePropertySchema(t, rawTableResponse, "columns").Type.Is(openapi3.TypeArray))
	require.True(t, requirePropertySchema(t, rawTableResponse, "rows").Type.Is(openapi3.TypeArray))
	requireResponseExample(t, rawTables, 200, "source_rows")

	rawTable400MediaType := responseJSONMediaType(t, rawTables, 400, "application/json")
	require.NotNil(t, rawTable400MediaType.Schema)
	require.NotNil(t, rawTable400MediaType.Schema.Value)
	require.Len(t, rawTable400MediaType.Schema.Value.OneOf, 2)
	var sawOperatorError, sawPlainError bool
	for _, candidate := range rawTable400MediaType.Schema.Value.OneOf {
		require.NotNil(t, candidate)
		require.NotNil(t, candidate.Value)
		if schemaLooksLikeOperatorErrorEnvelope(candidate.Value) {
			sawOperatorError = true
		}
		if schemaLooksLikePlainErrorEnvelope(candidate.Value) {
			sawPlainError = true
		}
	}
	require.True(t, sawOperatorError, "raw-table 400 should allow structured decode errors")
	require.True(t, sawPlainError, "raw-table 400 should allow the current plain validation error envelope")
	requireResponseExample(t, rawTables, 400, "invalid_identifier")
	requireResponseExample(t, rawTables, 400, "unknown_field")

	requirePlainErrorEnvelope(t, responseJSONSchema(t, rawTables, 403))
	requireResponseExample(t, rawTables, 403, "disabled")
	requireStructuredOperatorErrorEnvelope(t, responseJSONSchema(t, rawTables, 413))
	requireResponseExample(t, rawTables, 413, "request_body_too_large")

	metrics := doc.Paths.Find("/metrics").Get
	require.NotNil(t, metrics, "spec should document GET /metrics")
	metricsMediaType := responseJSONMediaType(t, metrics, 200, "text/plain")
	require.NotNil(t, metricsMediaType.Schema)
	require.NotNil(t, metricsMediaType.Schema.Value)
	require.True(t, metricsMediaType.Schema.Value.Type.Is(openapi3.TypeString))
	require.NotNil(t, metricsMediaType.Example)
}

func TestVerifyServiceOpenAPIStatefulNotesAndBoundaries(t *testing.T) {
	t.Parallel()

	doc := loadVerifyServiceOpenAPIDoc(t)
	require.Contains(t, doc.Info.Description, "retains only the most recent completed job")
	require.Contains(t, doc.Info.Description, "stateful")

	require.Nil(t, doc.Paths.Find("/healthz"), "runner health endpoint must not appear in the verify-service spec")
	require.Nil(t, doc.Paths.Find("/ingest/{mapping_id}"), "runner ingest endpoint must not appear in the verify-service spec")

	require.Len(t, doc.Paths.Map(), 5, "verify-service spec should only expose the five public verify endpoints")

	operatorError := doc.Components.Schemas["OperatorError"]
	require.NotNil(t, operatorError)
	require.NotNil(t, operatorError.Value)
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "unknown_field")
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "job_already_running")
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "job_not_found")
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "connection_failed")
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "mismatch_detected")
	require.Contains(t, operatorError.Value.Properties["code"].Value.Description, "verify_failed")
}

func loadVerifyServiceOpenAPIDoc(t *testing.T) *openapi3.T {
	t.Helper()

	specPath := locateRepoPath(t, "openapi", "verify-service.yaml")
	loader := openapi3.NewLoader()
	doc, err := loader.LoadFromFile(specPath)
	require.NoError(t, err)
	return doc
}

func requestJSONSchema(t *testing.T, operation *openapi3.Operation) *openapi3.Schema {
	t.Helper()

	require.NotNil(t, operation.RequestBody)
	require.NotNil(t, operation.RequestBody.Value)
	require.True(t, operation.RequestBody.Value.Required)

	mediaType := operation.RequestBody.Value.Content.Get("application/json")
	require.NotNil(t, mediaType, "request body should define application/json")
	require.NotNil(t, mediaType.Schema)
	require.NotNil(t, mediaType.Schema.Value)
	return mediaType.Schema.Value
}

func responseJSONSchema(t *testing.T, operation *openapi3.Operation, status int) *openapi3.Schema {
	t.Helper()

	mediaType := responseJSONMediaType(t, operation, status, "application/json")
	require.NotNil(t, mediaType.Schema)
	require.NotNil(t, mediaType.Schema.Value)
	return mediaType.Schema.Value
}

func responseJSONMediaType(t *testing.T, operation *openapi3.Operation, status int, mime string) *openapi3.MediaType {
	t.Helper()

	response := operation.Responses.Status(status)
	require.NotNil(t, response, "operation should define %d response", status)
	require.NotNil(t, response.Value)

	mediaType := response.Value.Content.Get(mime)
	require.NotNil(t, mediaType, "%d response should define %s", status, mime)
	return mediaType
}

func requirePropertySchema(t *testing.T, schema *openapi3.Schema, propertyName string) *openapi3.Schema {
	t.Helper()

	property := schema.Properties[propertyName]
	require.NotNil(t, property, "schema should define property %s", propertyName)
	require.NotNil(t, property.Value, "schema property %s should resolve", propertyName)
	return property.Value
}

func requireEnumStrings(t *testing.T, schema *openapi3.Schema, expected ...string) {
	t.Helper()

	actual := make([]string, 0, len(schema.Enum))
	for _, value := range schema.Enum {
		text, ok := value.(string)
		require.True(t, ok, "enum value should be a string: %v", value)
		actual = append(actual, text)
	}
	require.ElementsMatch(t, expected, actual)
}

func requireNoAdditionalProperties(t *testing.T, schema *openapi3.Schema) {
	t.Helper()

	require.NotNil(t, schema.AdditionalProperties.Has)
	require.False(t, *schema.AdditionalProperties.Has)
}

func schemaLooksLikeOperatorErrorEnvelope(schema *openapi3.Schema) bool {
	if schema == nil || !schema.Type.Is(openapi3.TypeObject) {
		return false
	}
	if len(schema.Required) != 1 || schema.Required[0] != "error" {
		return false
	}
	errorSchemaRef := schema.Properties["error"]
	if errorSchemaRef == nil || errorSchemaRef.Value == nil {
		return false
	}
	errorSchema := errorSchemaRef.Value
	if !errorSchema.Type.Is(openapi3.TypeObject) {
		return false
	}
	required := map[string]struct{}{}
	for _, name := range errorSchema.Required {
		required[name] = struct{}{}
	}
	for _, field := range []string{"category", "code", "message"} {
		if _, ok := required[field]; !ok {
			return false
		}
	}
	return true
}

func requirePlainErrorEnvelope(t *testing.T, schema *openapi3.Schema) {
	t.Helper()

	require.True(t, schemaLooksLikePlainErrorEnvelope(schema))
}

func schemaLooksLikePlainErrorEnvelope(schema *openapi3.Schema) bool {
	if schema == nil || !schema.Type.Is(openapi3.TypeObject) {
		return false
	}
	if len(schema.Required) != 1 || schema.Required[0] != "error" {
		return false
	}
	errorSchemaRef := schema.Properties["error"]
	if errorSchemaRef == nil || errorSchemaRef.Value == nil {
		return false
	}
	return errorSchemaRef.Value.Type.Is(openapi3.TypeString)
}

func requireStructuredOperatorErrorEnvelope(t *testing.T, schema *openapi3.Schema) {
	t.Helper()

	require.True(t, schema.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(t, []string{"error"}, schema.Required)

	errorSchema := requirePropertySchema(t, schema, "error")
	require.True(t, errorSchema.Type.Is(openapi3.TypeObject))
	require.ElementsMatch(t, []string{"category", "code", "message"}, errorSchema.Required)
	requireEnumStrings(
		t,
		requirePropertySchema(t, errorSchema, "category"),
		"request_validation",
		"job_state",
		"source_access",
		"mismatch",
		"verify_execution",
	)
	require.True(t, requirePropertySchema(t, errorSchema, "code").Type.Is(openapi3.TypeString))
	require.True(t, requirePropertySchema(t, errorSchema, "message").Type.Is(openapi3.TypeString))

	detailsSchema := requirePropertySchema(t, errorSchema, "details")
	require.True(t, detailsSchema.Type.Is(openapi3.TypeArray))
	require.NotNil(t, detailsSchema.Items)
	require.NotNil(t, detailsSchema.Items.Value)
	require.True(t, detailsSchema.Items.Value.Type.Is(openapi3.TypeObject))
}

func requireResponseExample(t *testing.T, operation *openapi3.Operation, status int, exampleName string) {
	t.Helper()

	response := operation.Responses.Status(status)
	require.NotNil(t, response)
	mediaType := response.Value.Content.Get("application/json")
	require.NotNil(t, mediaType)
	require.Contains(t, mediaType.Examples, exampleName)
	require.NotNil(t, mediaType.Examples[exampleName])
	require.NotNil(t, mediaType.Examples[exampleName].Value)
}
