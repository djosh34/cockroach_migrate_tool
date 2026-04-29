package verifyservice

import (
	"context"
	"encoding/json"
	"fmt"
	"regexp"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/lexbase"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
)

type RawTableSide string

const (
	RawTableSideSource      RawTableSide = "source"
	RawTableSideDestination RawTableSide = "destination"
)

var rawTableIdentifierPattern = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)

type RawTableRequest struct {
	Database string       `json:"database"`
	Side     RawTableSide `json:"side"`
	Schema   string       `json:"schema"`
	Table    string       `json:"table"`
}

type RawTableResponse struct {
	Database string           `json:"database"`
	Side     RawTableSide     `json:"side"`
	Schema   string           `json:"schema"`
	Table    string           `json:"table"`
	Columns  []string         `json:"columns"`
	Rows     []map[string]any `json:"rows"`
}

type RawTableReader interface {
	ReadRawTable(ctx context.Context, request RawTableRequest) (RawTableResponse, error)
}

type configBackedRawTableReader struct {
	config  Config
	connect connectFunc
}

type rawTableRequestError struct {
	message string
}

func (e rawTableRequestError) Error() string {
	return e.message
}

type rawTableReadError struct {
	message string
	cause   error
}

func (e rawTableReadError) Error() string {
	return e.message
}

func (e rawTableReadError) Unwrap() error {
	return e.cause
}

func newConfigBackedRawTableReader(cfg Config) RawTableReader {
	return configBackedRawTableReader{
		config:  cfg,
		connect: dbconn.Connect,
	}
}

func (r RawTableRequest) Validate() error {
	if r.Database == "" {
		return rawTableRequestError{message: "database must be set"}
	}
	if err := r.Side.Validate(); err != nil {
		return rawTableRequestError{message: err.Error()}
	}
	if err := validateRawTableIdentifier("schema", r.Schema); err != nil {
		return err
	}
	if err := validateRawTableIdentifier("table", r.Table); err != nil {
		return err
	}
	return nil
}

func (d RawTableSide) Validate() error {
	switch d {
	case RawTableSideSource, RawTableSideDestination:
		return nil
	default:
		return errors.New("side must be one of: source, destination")
	}
}

func (r configBackedRawTableReader) ReadRawTable(ctx context.Context, request RawTableRequest) (_ RawTableResponse, err error) {
	if err := request.Validate(); err != nil {
		return RawTableResponse{}, err
	}

	connStr, preferredID, err := r.connectionFor(request)
	if err != nil {
		return RawTableResponse{}, err
	}
	conn, err := r.connect(ctx, preferredID, connStr)
	if err != nil {
		return RawTableResponse{}, rawTableReadError{
			message: fmt.Sprintf("connect %s side for raw table output on database %s", request.Side, request.Database),
			cause:   err,
		}
	}
	defer func() {
		err = errors.CombineErrors(err, conn.Close(ctx))
	}()

	pgConn, ok := conn.(*dbconn.PGConn)
	if !ok {
		return RawTableResponse{}, rawTableReadError{
			message: fmt.Sprintf("raw table output requires postgres-compatible connection, got %T", conn),
		}
	}

	query := fmt.Sprintf(
		"SELECT * FROM %s.%s",
		lexbase.EscapeSQLIdent(request.Schema),
		lexbase.EscapeSQLIdent(request.Table),
	)
	rows, err := pgConn.Query(ctx, query)
	if err != nil {
		return RawTableResponse{}, rawTableReadError{
			message: fmt.Sprintf("query raw table %s.%s from %s side on database %s", request.Schema, request.Table, request.Side, request.Database),
			cause:   err,
		}
	}
	defer rows.Close()

	columns := make([]string, len(rows.FieldDescriptions()))
	for index, field := range rows.FieldDescriptions() {
		columns[index] = field.Name
	}

	rawRows := make([]map[string]any, 0)
	for rows.Next() {
		values, err := rows.Values()
		if err != nil {
			return RawTableResponse{}, rawTableReadError{
				message: fmt.Sprintf("read raw table row values for %s.%s from %s side on database %s", request.Schema, request.Table, request.Side, request.Database),
				cause:   err,
			}
		}
		row, err := normalizeRawTableRow(columns, values)
		if err != nil {
			return RawTableResponse{}, err
		}
		rawRows = append(rawRows, row)
	}
	if err := rows.Err(); err != nil {
		return RawTableResponse{}, rawTableReadError{
			message: fmt.Sprintf("iterate raw table rows for %s.%s from %s side on database %s", request.Schema, request.Table, request.Side, request.Database),
			cause:   err,
		}
	}

	return RawTableResponse{
		Database: request.Database,
		Side:     request.Side,
		Schema:   request.Schema,
		Table:    request.Table,
		Columns:  columns,
		Rows:     rawRows,
	}, nil
}

func (r configBackedRawTableReader) connectionFor(request RawTableRequest) (string, dbconn.ID, error) {
	pair, err := r.config.Verify.ResolveDatabase(request.Database)
	if err != nil {
		return "", "", rawTableRequestError{message: err.Error()}
	}

	switch request.Side {
	case RawTableSideSource:
		connStr, err := pair.Source.ConnectionString()
		return connStr, "source", err
	case RawTableSideDestination:
		connStr, err := pair.Destination.ConnectionString()
		return connStr, "target", err
	default:
		return "", "", rawTableRequestError{message: "side must be one of: source, destination"}
	}
}

func validateRawTableIdentifier(field string, value string) error {
	if !rawTableIdentifierPattern.MatchString(value) {
		return rawTableRequestError{
			message: fmt.Sprintf("%s must be a simple SQL identifier", field),
		}
	}
	return nil
}

func normalizeRawTableRow(columns []string, values []any) (map[string]any, error) {
	row := make(map[string]any, len(columns))
	for index, column := range columns {
		normalized, err := normalizeRawTableValue(column, values[index])
		if err != nil {
			return nil, err
		}
		row[column] = normalized
	}
	return row, nil
}

func normalizeRawTableValue(column string, value any) (any, error) {
	payload, err := json.Marshal(value)
	if err != nil {
		return nil, rawTableReadError{
			message: fmt.Sprintf("column %s is not JSON representable", column),
			cause:   err,
		}
	}
	var normalized any
	if err := json.Unmarshal(payload, &normalized); err != nil {
		return nil, rawTableReadError{
			message: fmt.Sprintf("column %s is not JSON representable", column),
			cause:   err,
		}
	}
	return normalized, nil
}
