package oracleconv

import (
	"database/sql"
	"strings"

	"github.com/lib/pq/oid"
)

func DataTypeToOID(
	typName string, dataPrecision sql.NullInt64, dataScale sql.NullInt64,
) (oid.Oid, bool) {
	typName = strings.ToUpper(typName)
	switch typName {
	case "INTEGER", "INT", "SIMPLE_INTEGER":
		return oid.T_oid, true
	case "SMALLINT":
		return oid.T_int2, true
	case "DEC", "NUMBER", "DECIMAL", "NUMERIC":
		if dataPrecision.Valid {
			prec := dataPrecision.Int64
			if dataScale.Valid {
				sc := dataScale.Int64
				if sc != 0 {
					return oid.T_numeric, true
				}
			}
			// Precision can be up to 38, len('9223372036854776000') == 19.
			// If we go higher, make it an int.
			if prec >= 19 {
				return oid.T_numeric, true
			}
			return oid.T_int8, true
		}
		return oid.T_numeric, true
	case "DOUBLE", "BINARY_DOUBLE":
		return oid.T_float8, true
	case "FLOAT", "BINARY_FLOAT", "REAL":
		return oid.T_float4, true
	case "LONG":
		return oid.T_int8, true
	case "BOOLEAN":
		return oid.T_bool, true
	case "DATE":
		return oid.T_date, true
	case "TIMESTAMP":
		if strings.HasSuffix(typName, "TIME ZONE") {
			return oid.T_timestamptz, true
		}
		return oid.T_timestamp, true
	case "TIMESTAMP_UNCONSTRAINED":
		return oid.T_timestamp, true
	case "TIMESTAMP_TZ_UNCONSTRAINED", "TIMESTAMP_LTZ_UNCONSTRAINED":
		return oid.T_timestamptz, true
	case "BLOB", "RAW":
		return oid.T_bytea, true
	case "CLOB":
		return oid.T_text, true
	case "NCHAR", "CHAR", "CHARACTER", "VARCHAR", "VARCHAR2", "NVARCHAR2":
		if strings.Contains(typName, "VAR") {
			return oid.T_varchar, true
		}
		return oid.T_char, true
	case "STRING":
		return oid.T_text, true
	}
	return 0, false
}
