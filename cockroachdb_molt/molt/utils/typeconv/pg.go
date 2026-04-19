package typeconv

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/types"
	"github.com/lib/pq/oid"
)

func (cm *PGColumnMeta) ToDefaultCRDBType(dataType, colName string) (*types.T, []*TypeConvError) {
	tfs := make([]*TypeConvError, 0)

	if cm.TypeOid == 0 {
		tfs = append(tfs, &TypeConvError{
			ShortDescription: OIDNotFoundForTypeFromPG,
			Message:          fmt.Sprintf("oid is not found for a pg column %s", colName),
		})
		return nil, tfs
	}
	defType, ok := types.OidToType[cm.TypeOid]
	if !ok {
		tfs = append(tfs, &TypeConvError{
			ShortDescription: UnknownTypeForOID,
			Message:          fmt.Sprintf("looking for corresponding type for oid %d from column %s", cm.TypeOid, colName),
			Blocking:         true,
		})
		return nil, tfs
	}

	switch defType.Family() {
	case types.TimestampFamily, types.TimestampTZFamily, types.TimeFamily, types.TimeTZFamily, types.IntervalFamily:
		datetimePrecision := cm.DatetimePrecision
		intervalTypeStr := cm.IntervalType
		if datetimePrecision == defDatetimePrecision && intervalTypeStr == "" {
			return defType, tfs
		}
		intervalType := int32(types.IntervalDurationType_UNSET)
		if intervalTypeStr != "" {
			intervalType, ok = types.IntervalDurationType_value[intervalTypeStr]
			if !ok {
				tfs = append(tfs, &TypeConvError{
					ShortDescription: UnknownIntervalType,
					Message:          fmt.Sprintf("interval type (%s) is not supported by CockroachDB", intervalTypeStr),
					Blocking:         true,
				})
				return nil, tfs
			}
		}
		switch defType.InternalType.Oid {
		case oid.T_timestamp:
			return types.MakeTimestamp(datetimePrecision), tfs
		case oid.T_timetz:
			return types.MakeTimeTZ(datetimePrecision), tfs
		case oid.T_timestamptz:
			return types.MakeTimestampTZ(datetimePrecision), tfs
		case oid.T_time:
			return types.MakeTime(datetimePrecision), tfs
		case oid.T_interval:
			intervalMeta := types.IntervalTypeMetadata{
				DurationField: types.IntervalDurationField{
					DurationType: types.IntervalDurationType(intervalType),
				},
			}
			if (intervalType == int32(types.IntervalDurationType_SECOND) ||
				intervalType == int32(types.IntervalDurationType_UNSET)) &&
				datetimePrecision != defDatetimePrecision {
				intervalMeta.Precision = datetimePrecision
				intervalMeta.PrecisionIsSet = true
			}
			return types.MakeInterval(intervalMeta), tfs
		default:
			return defType, tfs
		}
	case types.DecimalFamily:
		// In PG, numeric type must with precision >= 0. Scale can be negative/0/positive.
		// But CRDB only accept non-negative scale.
		// The numeric_scale column in information_schema.columns can be misleading.
		// Negative value is stored as positive value.
		// For example, NUMERIC(2, -3) is marked with precision 2 and scale 2045 in
		// information_schema.columns.
		// So we have to rely on dataType string.
		// If a type is a naked NUMERIC type, the dataType string will be "numeric".
		// Otherwise, if precision (and scale) are specified, the dataType string will be
		// "numeric(<precision>, <scale>)"
		precAndScaleStr := strings.TrimPrefix(dataType, "numeric")
		if precAndScaleStr == "" {
			return defType, nil
		}
		precAndScaleStr = precAndScaleStr[1 : len(precAndScaleStr)-1]
		precAndScaleList := strings.Split(precAndScaleStr, ",")
		if len(precAndScaleList) != 2 {
			tfs = append(tfs, &TypeConvError{
				ShortDescription: InvalidDecimalArgs,
				Message: fmt.Sprintf("cannot find precision and scale for column %s, "+
					"with received type expression %s",
					colName,
					dataType,
				),
				Blocking: true,
			})
		}
		precision, err := strconv.Atoi(precAndScaleList[0])
		if err != nil {
			tfs = append(tfs, &TypeConvError{
				ShortDescription: InvalidDecimalArgs,
				Message: fmt.Sprintf("cannot find precision for column %s expression %s",
					colName,
					precAndScaleList[0],
				),
				Blocking: true,
			})
		}

		scale, err := strconv.Atoi(precAndScaleList[1])
		if err != nil {
			tfs = append(tfs, &TypeConvError{
				ShortDescription: InvalidDecimalArgs,
				Message:          fmt.Sprintf("cannot find scale for column %s expression %s", colName, precAndScaleList[1]),
				Blocking:         true,
			})
		}

		if scale < 0 {
			tfs = append(tfs, &TypeConvError{
				ShortDescription: InvalidDecimalArgs,
				Message:          fmt.Sprintf("cockroachDB does not accept negative scale value %d for column %s, auto correcting scale to 0", scale, colName),
			})
			return types.MakeDecimal(int32(precision), 0), tfs
		}
		if scale > precision {
			tfs = append(tfs, &TypeConvError{
				ShortDescription: InvalidDecimalArgs,
				Message:          fmt.Sprintf("precision %d cannot be bigger than cm.NumScale %d for column %s", cm.NumPrecision, cm.NumScale, colName),
				Blocking:         true,
			})
			return nil, tfs
		}
		return types.MakeDecimal(int32(precision), int32(scale)), tfs
	default:
	}
	return defType, tfs
}

// PGColumnMeta collects the information about the column in a PostgreSQL table. This information are stored in the
// information_schema
type PGColumnMeta struct {
	TypeOid           oid.Oid
	ColumnDefault     string
	CharMaxLen        int
	CharOctetLen      int
	NumPrecision      int32
	NumPrecRadix      int32
	NumScale          int32
	DatetimePrecision int32
	IntervalType      string
	// TODO(janexing): handle collation
	CollationName string
}

// NewPGColumnMeta returns a new MyPGColumnMeta objects with unset/default value for all fields.
func NewPGColumnMeta() *PGColumnMeta {
	return &PGColumnMeta{
		NumPrecision:      defPlaceholder,
		NumScale:          defPlaceholder,
		DatetimePrecision: defDatetimePrecision,
		// TODO(janexing): accomodate char max len.
		CharMaxLen:   defPlaceholder,
		CharOctetLen: defPlaceholder,
	}
}

const defPlaceholder = -1
const defDatetimePrecision = 6
