package verifyservice

import (
	"errors"
	"fmt"
	"regexp"
	"strings"

	"github.com/cockroachdb/molt/utils"
)

type operatorError struct {
	category string
	code     string
	message  string
	details  []operatorErrorDetail
	cause    error
}

type operatorErrorDetail struct {
	Field  string `json:"field,omitempty"`
	Reason string `json:"reason,omitempty"`
}

type OperatorErrorDetail struct {
	Field  string `json:"field,omitempty"`
	Reason string `json:"reason,omitempty"`
}

type OperatorErrorView struct {
	Category string
	Code     string
	Message  string
	Details  []OperatorErrorDetail
}

type operatorErrorResponse struct {
	Error operatorErrorPayload `json:"error"`
}

type operatorErrorPayload struct {
	Category string                `json:"category"`
	Code     string                `json:"code"`
	Message  string                `json:"message"`
	Details  []operatorErrorDetail `json:"details,omitempty"`
}

var unknownFieldPattern = regexp.MustCompile(`^json: unknown field "([^"]+)"$`)
var embeddedURIPattern = regexp.MustCompile(`[a-zA-Z][a-zA-Z0-9+.-]*://[^\s"']+`)

var redactedDatabaseQueryParams = []string{
	"password",
	"pass",
	"passwd",
	"pwd",
	"sslpassword",
}

func newOperatorError(category string, code string, message string, details ...operatorErrorDetail) *operatorError {
	return &operatorError{
		category: category,
		code:     code,
		message:  message,
		details:  details,
	}
}

func newOperatorErrorWithCause(category string, code string, message string, cause error, details ...operatorErrorDetail) *operatorError {
	return &operatorError{
		category: category,
		code:     code,
		message:  message,
		details:  details,
		cause:    cause,
	}
}

func (e *operatorError) Error() string {
	if e == nil {
		return ""
	}
	if e.cause != nil {
		return fmt.Sprintf("%s: %v", e.message, e.cause)
	}
	if len(e.details) == 0 {
		return e.message
	}
	reasons := make([]string, 0, len(e.details))
	for _, detail := range e.details {
		switch {
		case detail.Field != "" && detail.Reason != "":
			reasons = append(reasons, fmt.Sprintf("%s %s", detail.Field, detail.Reason))
		case detail.Reason != "":
			reasons = append(reasons, detail.Reason)
		}
	}
	if len(reasons) == 0 {
		return e.message
	}
	return fmt.Sprintf("%s: %s", e.message, strings.Join(reasons, ", "))
}

func (e *operatorError) Unwrap() error {
	if e == nil {
		return nil
	}
	return e.cause
}

func (e *operatorError) payload() operatorErrorPayload {
	if e == nil {
		return operatorErrorPayload{}
	}
	return operatorErrorPayload{
		Category: e.category,
		Code:     e.code,
		Message:  sanitizeOperatorText(e.message),
		Details:  sanitizeOperatorErrorDetails(e.details),
	}
}

func (e *operatorError) view() OperatorErrorView {
	if e == nil {
		return OperatorErrorView{}
	}
	details := make([]OperatorErrorDetail, 0, len(e.details))
	for _, detail := range sanitizeOperatorErrorDetails(e.details) {
		details = append(details, OperatorErrorDetail{
			Field:  detail.Field,
			Reason: detail.Reason,
		})
	}
	return OperatorErrorView{
		Category: e.category,
		Code:     e.code,
		Message:  sanitizeOperatorText(e.message),
		Details:  details,
	}
}

func sanitizeOperatorErrorDetails(details []operatorErrorDetail) []operatorErrorDetail {
	if len(details) == 0 {
		return nil
	}

	sanitized := make([]operatorErrorDetail, 0, len(details))
	for _, detail := range details {
		sanitized = append(sanitized, operatorErrorDetail{
			Field:  sanitizeOperatorText(detail.Field),
			Reason: sanitizeOperatorText(detail.Reason),
		})
	}
	return sanitized
}

func sanitizeOperatorText(text string) string {
	return embeddedURIPattern.ReplaceAllStringFunc(text, func(raw string) string {
		trimmed, suffix := splitOperatorURISuffix(raw)
		sanitized, err := utils.SanitizeExternalStorageURI(trimmed, redactedDatabaseQueryParams)
		if err != nil {
			return raw
		}
		return sanitized + suffix
	})
}

func splitOperatorURISuffix(raw string) (string, string) {
	end := len(raw)
	for end > 0 {
		switch raw[end-1] {
		case '.', ',', ';', ')', ']', '}':
			end--
		default:
			return raw[:end], raw[end:]
		}
	}
	return raw, ""
}

func asOperatorError(err error) (*operatorError, bool) {
	var opErr *operatorError
	if err == nil {
		return nil, false
	}
	if ok := errors.As(err, &opErr); ok {
		return opErr, true
	}
	return nil, false
}

func classifyDecodeJSONError(err error) *operatorError {
	if err == nil {
		return nil
	}
	if errors.Is(err, errRequestBodyTooLarge) {
		return newOperatorError("request_validation", "request_body_too_large", errRequestBodyTooLarge.Error())
	}
	if matches := unknownFieldPattern.FindStringSubmatch(err.Error()); len(matches) == 2 {
		return newOperatorError(
			"request_validation",
			"unknown_field",
			"request body contains an unsupported field",
			operatorErrorDetail{
				Field:  matches[1],
				Reason: "unknown field",
			},
		)
	}
	if err.Error() == "request body must contain exactly one JSON object" {
		return newOperatorError("request_validation", "multiple_documents", err.Error())
	}
	return newOperatorErrorWithCause(
		"request_validation",
		"invalid_json",
		"request body is not valid JSON",
		err,
		operatorErrorDetail{Reason: err.Error()},
	)
}

func classifyRunFailure(err error) *operatorError {
	if opErr, ok := asOperatorError(err); ok {
		return opErr
	}
	return newOperatorError(
		"verify_execution",
		"verify_failed",
		fmt.Sprintf("verify execution failed: %v", err),
		operatorErrorDetail{Reason: err.Error()},
	)
}

func ExtractOperatorError(err error) (OperatorErrorView, bool) {
	opErr, ok := asOperatorError(err)
	if !ok {
		return OperatorErrorView{}, false
	}
	return opErr.view(), true
}
