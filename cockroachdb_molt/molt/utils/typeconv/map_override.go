package typeconv

import (
	"encoding/json"
	"fmt"
	"os"
	"slices"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/parser"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/types"
	"github.com/cockroachdb/errors"
	"github.com/rs/zerolog"
)

// tableTypeMapsJson correspond to json of the following format:
// [
//
//	{
//	  "table": "t1",
//	  "column_type_map": [
//	    {
//	      "column": "age",
//	      "source_type": "int",
//	      "crdb_type": "INT"
//	    },
//	    {
//	      "column": "name",
//	      "source_type": "varbit",
//	      "crdb_type": "string"
//	    }
//	  ]
//	}
//
// ]
type tableTypeMapsJson []*TableTypeMapJson

type TableTypeMapJson struct {
	TableName     string               `json:"table"`
	ColumnTypeMap []*ColumnTypeMapJson `json:"column_type_map"`
}

// Json fields must be exported so that json can be unmarshalled.
type ColumnTypeMapJson struct {
	ColumnName string `json:"column"`
	SourceType string `json:"source_type"`
	CRDBType   string `json:"crdb_type"`
}

func (m tableTypeMapsJson) String() (string, error) {
	res, err := json.Marshal(m)
	// This should not happen.
	if err != nil {
		return "", errors.AssertionFailedf("failed to marshal json from tableTypeMapsJson")
	}
	return string(res), nil
}

// TableTypeMap: table name -> ColumnTypeMap
type TableTypeMap map[string]ColumnTypeMap

func (ttm TableTypeMap) String() string {
	cnt := 0
	b := strings.Builder{}
	b.WriteString("[")

	var tableNames []string
	for k := range ttm {
		tableNames = append(tableNames, k)
	}
	slices.Sort(tableNames)

	for _, tableName := range tableNames {
		colMap := ttm[tableName]
		b.WriteString(fmt.Sprintf("{tableName:%s, %s}", tableName, colMap.String()))
		if cnt != len(ttm)-1 {
			b.WriteString(",")
		}
		cnt++
	}
	b.WriteString("]")
	return b.String()
}

// ColumnTypeMap: column name -> {source type: crdb type}.
type ColumnTypeMap map[string]*TypeKV

func (ctm ColumnTypeMap) String() string {
	cnt := 0
	b := strings.Builder{}
	b.WriteString("[")

	var colNames []string
	for k := range ctm {
		colNames = append(colNames, k)
	}
	slices.Sort(colNames)

	for _, colName := range colNames {
		kv := ctm[colName]
		b.WriteString(fmt.Sprintf("{colName:%s, mapping:%s}", colName, kv.String()))
		if cnt != len(ctm)-1 {
			b.WriteString(",")
		}
		cnt++
	}
	b.WriteString("]")
	return b.String()
}

// TypeKV: source type -> crdb type.
type TypeKV struct {
	sourceType string
	crdbType   *types.T
}

func (tkv *TypeKV) SourceType() string {
	return tkv.sourceType
}

func (tkv *TypeKV) CRDBType() *types.T {
	return tkv.crdbType
}

func (tkv TypeKV) String() string {
	return fmt.Sprintf("{%s:{%s}}", tkv.sourceType, strings.TrimSuffix(tkv.crdbType.DebugString(), " "))
}

// toTableTypeMap is to converted the marshalled "json" struct to the map struct.
func (ms tableTypeMapsJson) toTableTypeMap() (TableTypeMap, error) {
	res := make(TableTypeMap)
	for _, m := range ms {
		if existingMap, ok := res[m.TableName]; ok {
			return nil, errors.AssertionFailedf("mapping rule for table %q has been defined: %s", m.TableName, existingMap)
		}
		mapForTable := make(ColumnTypeMap)
		res[m.TableName] = mapForTable
		for _, col := range m.ColumnTypeMap {
			if rule, ok := mapForTable[col.ColumnName]; ok {
				return nil, errors.AssertionFailedf("mapping rule for column %q from table %q has been defined: %s", col.ColumnName, m.TableName, rule)
			}
			if col.SourceType == "" || col.CRDBType == "" {
				return nil, errors.AssertionFailedf("mapping rule for column %q from table %q is not found", col.ColumnName, m.TableName)
			}
			crdbType, err := getTypeFromName(strings.ToLower(col.CRDBType))
			if err != nil {
				return nil, errors.Wrapf(err, "cannot get the crdb type for %q", col.CRDBType)
			}
			mapForTable[col.ColumnName] = &TypeKV{
				sourceType: strings.ToLower(col.SourceType),
				crdbType:   crdbType,
			}
		}
	}
	return res, nil
}

func getOverrideTypeMapFromJsonBytes(
	bytesValus []byte, logger zerolog.Logger,
) (TableTypeMap, error) {
	var jsonRes = tableTypeMapsJson{}
	if err := json.Unmarshal(bytesValus, &jsonRes); err != nil {
		return nil, errors.Wrapf(err, "unable to unmarshal json mapping")
	}
	for _, tableRes := range jsonRes {
		if tableRes.TableName == "" {
			return nil, errors.AssertionFailedf("table name is not specified for overriding type map")
		}
		for _, colRes := range tableRes.ColumnTypeMap {
			if colRes.ColumnName == "" {
				return nil, errors.AssertionFailedf("column name is not specified for overriding type map")
			}
			if colRes.SourceType == "" || colRes.CRDBType == "" {
				return nil, errors.AssertionFailedf("mapping rule for column %s from table %s is not found", colRes.ColumnName, tableRes.TableName)
			}
			if colRes.ColumnName == "*" {
				logger.Warn().Msgf("the type mapping will apply to all columns of table %s", tableRes.TableName)
			}
		}
	}
	jsonResStr, err := jsonRes.String()
	if err != nil {
		return nil, err
	}
	logger.Debug().Msgf("received type mapping: %s", jsonResStr)

	res, err := jsonRes.toTableTypeMap()
	if err != nil {
		return nil, err
	}
	logger.Info().Msgf("converted type mapping: %s", res)
	return res, nil
}

func GetOverrideTypeMapFromFile(filepath string, logger zerolog.Logger) (TableTypeMap, error) {
	bytesValus, err := os.ReadFile(filepath)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to read json file %s for type mapping", filepath)
	}
	res, err := getOverrideTypeMapFromJsonBytes(bytesValus, logger)
	if err != nil {
		return nil, errors.WithHintf(err, "is the json file %s is of the correct format?", filepath)
	}
	return res, nil
}

func getTypeFromName(typ string) (*types.T, error) {
	stmt, err := parser.Parse(fmt.Sprintf("SELECT 1::%s", typ))
	if err != nil {
		return nil, err
	}

	ast, ok := stmt[0].AST.(*tree.Select)
	if !ok {
		return nil, errors.AssertionFailedf("failed to assert ast as tree.Select")
	}
	selectCaluse, ok := ast.Select.(*tree.SelectClause)
	if !ok {
		return nil, errors.AssertionFailedf("failed to assert for tree.SelectClause")
	}
	if len(selectCaluse.Exprs) == 0 {
		return nil, errors.AssertionFailedf("failed to find expression from select clause")
	}
	castExpr, ok := selectCaluse.Exprs[0].Expr.(*tree.CastExpr)
	if !ok {
		return nil, errors.AssertionFailedf("failed to assert the cast expression")
	}
	res, ok := castExpr.Type.(*types.T)
	if !ok {
		return nil, errors.AssertionFailedf("cannot assert the type %q as *types.T on crdb", typ)
	}

	return res, nil
}
