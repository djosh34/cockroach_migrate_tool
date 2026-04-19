package fetch

import (
	"bytes"
	"context"
	"fmt"
	"regexp"
	"sort"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/parser"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"github.com/cockroachdb/molt/dbtable"
	"github.com/cockroachdb/molt/utils/typeconv"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

type columnsWithType []columnWithType

// CRDBCreateTableStmt returns a create table statement string with columnsWithType
// as the column clause.
func (cs columnsWithType) CRDBCreateTableStmt(
	logger zerolog.Logger, table dbtable.DBTable, overridingTypeMap typeconv.ColumnTypeMap,
) (string, error) {
	tName, err := parser.ParseQualifiedTableName(table.String())
	if err != nil {
		return "", err
	}
	res := tree.CreateTable{
		Table: *tName,
	}

	pkList := columnsWithType{}
	for _, col := range cs {
		if col.pkPos != 0 {
			pkList = append(pkList, col)
		}
	}

	// If there is only one pk, we simply need to park this particular column as
	// pk. If there are more than one pks, we need to create a pk constraint
	// that group all the selected columns, thus result in different syntax.
	includePkForEachCol := len(pkList) <= 1

	for _, col := range cs {
		if table.LocalityExpr == multiRegionRBRLocality && col.columnName == multiRegionHiddenColName {
			// We don't explicitly create `crdb_region` table for RBR tables but let it be created
			// automatically via the `LOCALITY REGIONAL BY ROW` clause.
			logger.Info().Msgf("skipping creation of the hidden default column %s for table %s with locality %s", multiRegionHiddenColName, table.String(), table.LocalityExpr)
			continue
		}
		var mappingRule *typeconv.TypeKV
		if overridingTypeMap != nil {
			mappingRule = overridingTypeMap["*"]
			colSpecificRule, ok := overridingTypeMap[col.columnName]
			if ok {
				mappingRule = colSpecificRule
			}
		}
		colDef, err := col.CRDBColDef(includePkForEachCol, logger, mappingRule)
		if err != nil {
			return "", err
		}
		res.Defs = append(res.Defs, colDef)
	}

	if !includePkForEachCol {
		pkColNode := tree.IndexElemList{}
		pkList.sortByPkPos()
		for _, pk := range pkList {
			pkColNode = append(pkColNode, tree.IndexElem{Column: tree.Name(pk.columnName)})
		}
		res.Defs = append(res.Defs, &tree.UniqueConstraintTableDef{
			PrimaryKey: true,
			IndexTableDef: tree.IndexTableDef{
				Name:    "primary",
				Columns: pkColNode,
			},
		})
	}

	createTableStr := res.String()
	if table.LocalityExpr != "" {
		createTableStr = strings.Join([]string{createTableStr, table.LocalityExpr}, " LOCALITY ")
	}

	return createTableStr, nil
}

type columnWithType struct {
	table *dbtable.DBTable

	// The following fields are shared information for columns from all dialects.
	schemaName      string
	tableName       string
	columnName      string
	dataType        string
	columnType      string
	nullable        bool
	pkPos           int
	udtName         string
	udtDefinition   string
	arrDim          int
	ordinalPosition int

	pgMeta *typeconv.PGColumnMeta

	// mysqlMeta stores the mysql column dedicated information.
	mysqlMeta *typeconv.MySQLColumnMeta
}

func (cs columnsWithType) sortByPkPos() {
	// Define a custom sorting function for sorting by pkPos in ascending order.
	sort.Slice(cs, func(i, j int) bool {
		return cs[i].pkPos < cs[j].pkPos
	})
}

func (t *columnWithType) CRDBColDef(
	includePk bool, logger zerolog.Logger, overrideTypeMap *typeconv.TypeKV,
) (*tree.ColumnTableDef, error) {
	var colType tree.ResolvableTypeReference
	var err error
	var scs []*typeconv.TypeConvError
	if t.udtDefinition != "" {
		if t.udtName == "" {
			// This should not happen, but as a sanity check.
			return nil, errors.AssertionFailedf("user defined type definition [%s] is not null, but the type name is null", t.udtDefinition)
		}
		colType, err = tree.NewUnresolvedObjectName(1 /* numParts */, [3]string{t.udtName, "", ""}, tree.NoAnnotation /* annotation idx */)
		if err != nil {
			return nil, errors.Wrapf(err, "unable to parse the type name [%s]", t.udtName)
		}
	} else {
		// Prioritize the overriding rules for type mapping.
		if overrideTypeMap != nil {
			logger.Info().Msgf("received overlapping map rule for column %s: %s", t.Name(), overrideTypeMap.String())
			if !strings.EqualFold(t.dataType, overrideTypeMap.SourceType()) {
				logger.Warn().Msgf("the original type of column %s is %s, while overriding map has source type %s, so igoring the overriding map", t.Name(), t.dataType, overrideTypeMap.SourceType())
			} else {
				colType = overrideTypeMap.CRDBType()
				logger.Info().Msgf("using customized mapping for column %s: %s -> %s", t.Name(), t.dataType, colType.SQLString())
			}
		}

		if colType == nil {
			if t.pgMeta != nil {
				// If this is from a PG source.
				colType, scs = t.pgMeta.ToDefaultCRDBType(t.dataType, t.columnName)
			} else if t.mysqlMeta != nil {
				// If this is from a mysql source.
				colType, scs = t.mysqlMeta.ToDefaultCRDBType(t.dataType, t.columnName)
			} else {
				return nil, errors.AssertionFailedf("cant find either pgmeta or mysqlmeta for column %s", t.Name())
			}
		}

		var err error
		for _, sc := range scs {
			if sc.Blocking {
				err = errors.CombineErrors(err, errors.Newf(
					"failed to get crdb type from type [%s] for column %s: %s",
					t.dataType,
					t.Name(),
					sc.ShortDescription,
				))
			}
			logger.Warn().Msgf("type [%s] for column %s: %s", t.dataType, t.Name(), sc.ShortDescription)
		}
		if err != nil {
			log.Err(err)
			return nil, err
		}
	}

	res := &tree.ColumnTableDef{
		Name: tree.Name(t.columnName),
		Type: colType,
	}
	if t.nullable {
		res.Nullable.Nullability = tree.SilentNull
	}
	if t.pkPos != 0 && includePk {
		res.PrimaryKey.IsPrimaryKey = true
	}
	return res, nil
}

func (t *columnWithType) Name() string {
	return fmt.Sprintf("%s.%s", t.table.String(), t.columnName)
}

func (t *columnWithType) String() string {
	return fmt.Sprintf("column:%s, type:%s, nullable:%t, pkpos:%d",
		t.Name(), t.dataType, t.nullable, t.pkPos)
}

func GetColumnTypes(
	ctx context.Context, logger zerolog.Logger, conn dbconn.Conn, table dbtable.DBTable,
) (columnsWithType, error) {
	const (
		pgQuery = `SELECT DISTINCT
    t1.schema_name,
    t1.table_name,
    t1.column_name,
    t1.data_type,
    t1.type_oid,
    t1.nullable,
    COALESCE(t1.pk_pos, 0) as pk_pos,
    t1.arr_dim,
    COALESCE(t2.udt_name, '') AS enum_type,
    COALESCE(t2.udt_def, '') AS enum_type_definition,
    t2.ordinal_position,
	COALESCE(t2.numeric_scale, 0) AS numeric_scale,
	COALESCE(t2.numeric_precision, 0) AS numeric_precision,
	COALESCE(t2.numeric_precision_radix, 0) AS numeric_precision_radix,
	COALESCE(t2.datetime_precision, 0) AS datetime_precision,
	COALESCE(t2.interval_type, '') AS interval_type,
	COALESCE(t2.collation_name, '') AS collation_name
FROM (
    SELECT
        c.relnamespace::regnamespace::text AS schema_name,
        c.relname AS table_name,
        a.attname AS column_name,
        format_type(a.atttypid, a.atttypmod) AS data_type,
        a.atttypid AS type_oid,
        NOT a.attnotnull AS nullable,
        CASE
        WHEN a.attname IN ( -- Check if the column is in the pk keys for given table.
            SELECT ccu.column_name
            FROM information_schema.constraint_column_usage ccu
            JOIN pg_catalog.pg_index ix ON ix.indrelid = a.attrelid AND ix.indisprimary
            WHERE constraint_name = (
                SELECT constraint_name
                FROM information_schema.table_constraints
                WHERE table_name = $1 AND table_schema = $2
                AND constraint_type = 'PRIMARY KEY'
            ) AND a.attnum = ANY(ix.indkey) -- Exclude implicit columns such as crdb_region.
        ) THEN
            ( -- If so, get its ordinal position in the pk keys.
            SELECT kcu.ordinal_position
             FROM information_schema.key_column_usage kcu
             JOIN information_schema.table_constraints tc ON 
                 kcu.constraint_name = tc.constraint_name  
                     AND kcu.table_name = tc.table_name 
                     AND kcu.table_schema = tc.table_schema
             WHERE kcu.table_name = c.relname
               AND kcu.table_schema = $2
               AND kcu.column_name = a.attname
               AND tc.constraint_type = 'PRIMARY KEY'
            )
        ELSE 0
        END AS pk_pos,
        a.attndims AS arr_dim
    FROM
        pg_catalog.pg_class c
    JOIN pg_catalog.pg_attribute a ON c.oid = a.attrelid
    WHERE
        c.relkind = 'r'  -- 'r' indicates a table (relation)
        AND a.attnum > 0 -- Exclude system columns
        AND c.relname = $1
        AND c.relnamespace::regnamespace::text = $2
        AND a.attisdropped = false -- Exclude dropped columns
) t1
LEFT JOIN (
    SELECT
        c.column_name,
        c.table_name,
        c.table_schema,
        c.udt_name,
        c.ordinal_position,
        c.numeric_scale,
        c.numeric_precision,
        c.numeric_precision_radix,
        c.datetime_precision,
        c.interval_type,
        c.collation_name,
        t.definition AS udt_def
    FROM
        information_schema.columns c
    LEFT JOIN (
        SELECT
            'CREATE TYPE IF NOT EXISTS ' || t.typname || ' AS ENUM ' ||
            '(' || string_agg(quote_literal(e.enumlabel), ', ' ORDER BY e.enumsortorder) || ');' AS definition,
            t.typname
        FROM
            pg_type t
        JOIN pg_enum e ON t.oid = e.enumtypid
        GROUP BY
            t.typname
    ) t ON c.udt_name = t.typname
    WHERE
        c.table_name = $1 AND c.table_schema = $2
) t2 ON t1.column_name = t2.column_name
    AND t1.table_name = t2.table_name
    AND t1.schema_name = t2.table_schema
ORDER BY
    t1.schema_name,
    t1.table_name,
    t2.ordinal_position;
`
		mysqlQuery = `SELECT 
    c.TABLE_SCHEMA, 
    c.TABLE_NAME, 
    c.COLUMN_NAME, 
    c.DATA_TYPE,
    COALESCE(c.CHARACTER_MAXIMUM_LENGTH, '-1') AS CHARACTER_MAXIMUM_LENGTH,
    COALESCE(c.CHARACTER_OCTET_LENGTH,'-1') AS CHARACTER_OCTET_LENGTH,
    COALESCE(c.COLUMN_TYPE, '') AS COLUMN_TYPE,
    COALESCE(c.NUMERIC_PRECISION,'-1') AS NUMERIC_PRECISION,
    COALESCE(c.NUMERIC_SCALE,'-1') AS NUMERIC_SCALE,
    COALESCE(c.DATETIME_PRECISION,'-1') AS DATETIME_PRECISION,
    COALESCE(c.CHARACTER_SET_NAME,'') AS CHARACTER_SET_NAME,
    COALESCE(c.COLLATION_NAME,'') AS COLLATION_NAME,
    COALESCE(c.COLUMN_DEFAULT, '') AS COLUMN_DEFAULT,
    CASE 
        WHEN c.IS_NULLABLE = 'YES' THEN 'TRUE'
        ELSE 'FALSE' 
    END AS NULLABLE,
    IFNULL(pk.position_in_pk, 0) AS pk_pos
FROM 
    information_schema.COLUMNS c
LEFT JOIN (
    SELECT
        kcu.TABLE_SCHEMA,
        kcu.TABLE_NAME,
        kcu.COLUMN_NAME,
        kcu.ORDINAL_POSITION AS position_in_pk
    FROM
        information_schema.TABLE_CONSTRAINTS AS tc
    JOIN information_schema.KEY_COLUMN_USAGE AS kcu ON
       tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME AND tc.TABLE_NAME = kcu.TABLE_NAME AND tc.TABLE_SCHEMA = kcu.TABLE_SCHEMA
    WHERE
        tc.CONSTRAINT_TYPE = 'PRIMARY KEY'
) AS pk ON c.TABLE_SCHEMA = pk.TABLE_SCHEMA AND c.TABLE_NAME = pk.TABLE_NAME AND c.COLUMN_NAME = pk.COLUMN_NAME
JOIN 
    information_schema.TABLES t ON c.TABLE_SCHEMA = t.TABLE_SCHEMA AND c.TABLE_NAME = t.TABLE_NAME
WHERE 
    c.TABLE_SCHEMA = DATABASE() 
    AND t.TABLE_TYPE = 'BASE TABLE'
    AND c.TABLE_NAME = '%s'
ORDER BY 
    c.TABLE_SCHEMA, 
    c.TABLE_NAME,  
    c.ORDINAL_POSITION;
`
	)

	res := make([]columnWithType, 0)
	logger.Info().Msgf("getting column types for table: %s", table.String())

	switch conn := conn.(type) {
	case *dbconn.PGConn:
		var crdbExtraCheckConn *dbconn.PGConn
		if conn.IsCockroach() {
			newConn, err := conn.Clone(ctx)
			if err != nil {
				return nil, errors.Wrapf(err, "failed to clone conn for check hidden column")
			}
			crdbExtraCheckConn = newConn.(*dbconn.PGConn)
		}
		defer func() {
			if crdbExtraCheckConn != nil {
				crdbExtraCheckConn.Close(ctx)
			}
		}()

		rows, err := conn.Query(ctx, pgQuery, table.Table, table.Schema)
		if err != nil {
			return nil, err
		}

		defer func() { rows.Close() }()

		for rows.Next() {
			pgMt := typeconv.NewPGColumnMeta()
			newCol := columnWithType{table: &table, pgMeta: pgMt}
			if err := rows.Scan(
				&newCol.schemaName,
				&newCol.tableName,
				&newCol.columnName,
				&newCol.dataType,
				&newCol.pgMeta.TypeOid,
				&newCol.nullable,
				&newCol.pkPos,
				&newCol.arrDim,
				&newCol.udtName,
				&newCol.udtDefinition,
				&newCol.ordinalPosition,
				&newCol.pgMeta.NumScale,
				&newCol.pgMeta.NumPrecision,
				&newCol.pgMeta.NumPrecRadix,
				&newCol.pgMeta.DatetimePrecision,
				&newCol.pgMeta.IntervalType,
				&newCol.pgMeta.CollationName,
			); err != nil {
				return nil, errors.Wrap(err, "failed to scan query result to a columnWithType object")
			}
			logger.Debug().Msgf("collected column: %s", newCol.String())
			if newCol.arrDim > 1 {
				return nil, errors.Newf("original column %s of table %s.%s is nested array, which is currently not supported by CockroachDB\nSee also: https://github.com/cockroachdb/cockroach/issues/32552", newCol.columnName, newCol.schemaName, newCol.tableName)
			}

			if conn.IsCockroach() {
				if crdbExtraCheckConn == nil {
					return nil, errors.AssertionFailedf("crdbExtraCheckConn is null")
				}
				const defaultCRDBPKName = `rowid`
				const checkHiddenQ = "SELECT hidden FROM crdb_internal.table_columns WHERE descriptor_id = $1::regclass::oid AND column_name = $2"
				var hiddenRes bool
				if checkHiddenErr := crdbExtraCheckConn.QueryRow(ctx, checkHiddenQ, table.String(), newCol.columnName).Scan(&hiddenRes); checkHiddenErr != nil {
					return nil, errors.Wrapf(checkHiddenErr, "failed to check if the column is hidden for crdb source")
				}
				if hiddenRes && newCol.columnName == defaultCRDBPKName {
					continue
				}
			}

			res = append(res, newCol)
		}
	case *dbconn.MySQLConn:
		q := fmt.Sprintf(mysqlQuery, table.Table)
		rows, err := conn.Query(q)
		if err != nil {
			return nil, err
		}
		for rows.Next() {
			mysqlMt := &typeconv.MySQLColumnMeta{}
			newCol := columnWithType{table: &table, mysqlMeta: mysqlMt}
			if err := rows.Scan(
				&newCol.schemaName,
				&newCol.tableName,
				&newCol.columnName,
				&newCol.dataType,
				&mysqlMt.CharMaxLen,
				&mysqlMt.CharOctetLen,
				&mysqlMt.ColumnType,
				&mysqlMt.NumPrecision,
				&mysqlMt.NumScale,
				&mysqlMt.DatetimePrecision,
				&mysqlMt.CharSetName,
				&mysqlMt.CollationName,
				&mysqlMt.ColumnDefault,
				&newCol.nullable,
				&newCol.pkPos,
			); err != nil {
				return nil, errors.Wrap(err, "failed to scan query result to a columnWithType object")
			}
			logger.Debug().Msgf("collected column: %s", newCol.String())

			if strings.ToLower(newCol.dataType) == enumTypeStr {
				udtDefinition, udtName, getUdtErr := convertMySQLEnum(newCol)
				if getUdtErr != nil {
					return nil, getUdtErr
				}
				newCol.udtDefinition = udtDefinition
				newCol.udtName = udtName
			}
			res = append(res, newCol)
		}
	default:
		return nil, errors.New("not supported conn type")
	}

	logger.Info().Msgf("finished getting column types for table: %s", table.String())
	return res, nil
}

func GetDropTableStmt(table dbtable.DBTable) (string, error) {
	tName, err := parser.ParseQualifiedTableName(table.String())
	if err != nil {
		return "", err
	}
	res := tree.DropTable{
		Names:    tree.TableNames{*tName},
		IfExists: true,
	}

	return res.String(), nil
}

func GetTypeConvMap(
	customizedTypeMapPath string, logger zerolog.Logger,
) (typeconv.TableTypeMap, error) {
	if customizedTypeMapPath == "" {
		return nil, nil
	}
	overrideTypeMap, err := typeconv.GetOverrideTypeMapFromFile(customizedTypeMapPath, logger)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to get the overriding type map for schema creation")
	}
	return overrideTypeMap, nil
}

type crdbRegion string

type crdbRegionsWithAttr map[crdbRegion]*crdbRegionAttrs

type crdbRegionAttrs struct {
	zones           []string
	dbName          string
	primaryRegion   bool
	secondaryRegion bool
}

func maybeCheckTableLocality(
	ctx context.Context, logger zerolog.Logger, conn dbconn.Conn, table *dbtable.DBTable,
) error {
	if !conn.IsCockroach() {
		return nil
	}
	crdbConn := conn.(*dbconn.PGConn)
	var tableLocalityRes string
	if err := crdbConn.QueryRow(ctx, `SELECT COALESCE(locality, '') FROM crdb_internal.tables WHERE table_id = $1::regclass::oid`, table.String()).Scan(&tableLocalityRes); err != nil {
		return errors.Wrapf(err, "failed to scan result for table locality")
	}
	table.LocalityExpr = tableLocalityRes
	return nil
}

func MaybeCheckMultiRegionLocality(
	ctx context.Context, logger zerolog.Logger, conns dbconn.OrderedConns, table *dbtable.DBTable,
) (checked bool, err error) {
	srcConn := conns[0]
	tgtConn := conns[1]
	if !srcConn.IsCockroach() {
		return false, nil
	}
	if !tgtConn.IsCockroach() {
		return false, errors.AssertionFailedf("source is cockroachdb while target is not cockroachdb")
	}
	// If this is not a multi-region table, no-op.
	if table.LocalityExpr == "" {
		return false, nil
	}
	newSrcConn, err := srcConn.Clone(ctx)
	if err != nil {
		return false, errors.Wrapf(err, "failed to clone source conn to check locality")
	}
	defer func() { newSrcConn.Close(ctx) }()
	checkMRSrcConn := newSrcConn.(*dbconn.PGConn)

	newTgtConn, err := tgtConn.Clone(ctx)
	if err != nil {
		return false, errors.Wrapf(err, "failed to clone target conn to check locality")
	}
	defer func() { newTgtConn.Close(ctx) }()

	checkMRTgtConn := newTgtConn.(*dbconn.PGConn)

	srcRegionsWithAttr := make(crdbRegionsWithAttr)
	tgtRegionsWithAttr := make(crdbRegionsWithAttr)

	regionsWithAttrList := []crdbRegionsWithAttr{srcRegionsWithAttr, tgtRegionsWithAttr}

	for i, conn := range []*dbconn.PGConn{
		checkMRSrcConn,
		checkMRTgtConn,
	} {
		err = func() error {
			const (
				checkRegionsQ = `SHOW REGIONS FROM DATABASE %s;`
				currentDBQ    = `SELECT current_database()`
			)

			var currentDBName string
			if err := conn.QueryRow(ctx, currentDBQ).Scan(&currentDBName); err != nil {
				return errors.Wrapf(err, "failed to get the current db name for conn idx [%d]", i)
			}

			rows, err := conn.Query(ctx, fmt.Sprintf(checkRegionsQ, currentDBName))
			if err != nil {
				return errors.Wrapf(err, "failed checking the regions for conn idx [%d]", i)
			}
			defer func() { rows.Close() }()
			for rows.Next() {
				var regionName crdbRegion
				regionAttrs := crdbRegionAttrs{}
				if err := rows.Scan(&regionAttrs.dbName, &regionName, &regionAttrs.primaryRegion, &regionAttrs.secondaryRegion, &regionAttrs.zones); err != nil {
					return errors.Wrapf(err, "failed to scan results from show regions for conn idx [%d]", i)
				}
				regionsWithAttrList[i][regionName] = &regionAttrs
			}
			return nil
		}()
		if err != nil {
			return false, err
		}
	}
	// Need to check if the regions of the target db contains the ones from the source.
	// We don't compare the zone configs or primary and secondary regions because they don't really matter for
	// RBR tables.
	for regionOfSrc, attrOfSrc := range srcRegionsWithAttr {
		if attrOfSrc == nil {
			return false, errors.AssertionFailedf("nil attr for source db for region %s", regionOfSrc)
		}
		attrOfTgt, ok := tgtRegionsWithAttr[regionOfSrc]
		if !ok {
			return true, errors.AssertionFailedf("source crdb contains region %s but target crdb doesn't contain", regionOfSrc)
		}
		if attrOfTgt == nil {
			return false, errors.AssertionFailedf("nil attr for target db for region %s", regionOfSrc)
		}
	}

	return true, nil
}

const (
	multiRegionHiddenColName = `crdb_region`
	multiRegionRBRLocality   = `REGIONAL BY ROW`
)

func GetCreateTableStmt(
	ctx context.Context,
	logger zerolog.Logger,
	conns dbconn.OrderedConns,
	table dbtable.DBTable,
	overrideTypeMap typeconv.TableTypeMap,
) (string, error) {

	srcConn := conns[0]
	newCols, err := GetColumnTypes(ctx, logger, srcConn, table)
	if err != nil {
		return "", errors.Wrapf(err, "failed get columns for target table: %s", table.String())
	}

	var res string

	for _, col := range newCols {
		if col.udtDefinition != "" {
			if col.columnName == multiRegionHiddenColName && table.LocalityExpr == multiRegionRBRLocality {
				logger.Info().Msgf("skipping creation of the enum for hidden default column %s, for table %s, with locality %s", multiRegionHiddenColName, table.String(), table.LocalityExpr)
				continue
			}
			logger.Info().Msgf("the original schema contains enum type [%s]. A tentative enum type will be created as [%s]", col.udtName, col.udtDefinition)
			res = strings.Join([]string{res, col.udtDefinition}, " ")
		}
	}

	var colTypeMap typeconv.ColumnTypeMap
	if overrideTypeMap != nil {
		colTypeMap = overrideTypeMap[table.String()]
	}

	createTableStmt, err := newCols.CRDBCreateTableStmt(logger, table, colTypeMap)
	if err != nil {
		return "", err
	}

	if res != "" {
		return strings.TrimPrefix(strings.Join([]string{res, createTableStmt}, " "), " "), nil
	}
	return createTableStmt, nil
}

const enumTypeStr = "enum"

func convertMySQLEnum(
	newCol columnWithType,
) (createEnumStmt string, enumTypeName string, err error) {
	if newCol.mysqlMeta == nil || newCol.mysqlMeta.ColumnType == "" {
		return "", "", errors.Newf("original type is enum but with empty column type definition")
	}
	enumTypeName = fmt.Sprintf("%s_%s_%s_enum", newCol.schemaName, newCol.tableName, newCol.columnName)
	pattern := regexp.MustCompile(`enum(\(.+\))`)
	createEnumStmt = newCol.mysqlMeta.ColumnType
	matches := pattern.FindAllStringSubmatch(createEnumStmt, -1)
	if len(matches) == 0 {
		return "", "", errors.Newf("cannot extract enum values from the original enum expression: [%s]", newCol.columnType)
	}
	for _, match := range matches {
		if len(match) < 2 {
			return "", "", errors.Newf("cannot extract enum values from matched enum expression: [%s]", match)
		}
		enumValues := match[1]
		output := fmt.Sprintf("CREATE TYPE IF NOT EXISTS %s AS ENUM %s;", enumTypeName, enumValues)
		createEnumStmt = pattern.ReplaceAllString(createEnumStmt, output)
	}
	return createEnumStmt, enumTypeName, nil
}

type constraints []string

type constraintsWithTable struct {
	table dbtable.DBTable
	cons  constraints
}

func (ct *constraintsWithTable) String() string {
	var b bytes.Buffer
	b.WriteString(fmt.Sprintf("table: %s,", ct.table))
	for i, con := range ct.cons {
		b.WriteString(con)
		if i != len(ct.cons)-1 {
			b.WriteString(",")
		}
	}
	return b.String()
}

func GetConstraints(
	ctx context.Context, logger zerolog.Logger, conn dbconn.Conn, table dbtable.DBTable,
) ([]string, error) {
	const (
		pgQuery = `SELECT         
        pg_catalog.pg_get_constraintdef(c.oid) AS constraint_def
        FROM pg_catalog.pg_class s
        JOIN pg_catalog.pg_constraint c ON (s.oid = c.conrelid)
        WHERE conparentid = 0 
          AND s.relkind = 'r' -- 'r' indicates a table (relation)
          AND c.contype != 'p' -- 'p' indicates a primary key constraint
          AND s.relname= $1
          AND s.relnamespace::regnamespace::text = $2 
        ORDER BY conrelid, conname;`
		mysqlQuery = `SHOW CREATE TABLE %s`
	)

	var res []string
	switch conn := conn.(type) {
	case *dbconn.PGConn:
		rows, err := conn.Query(ctx, pgQuery, table.Table, table.Schema)
		if err != nil {
			return nil, errors.Wrapf(err, "failed to get the constraints for table %s", table.Table)
		}
		for rows.Next() {
			var constraintStmt string
			if err := rows.Scan(&constraintStmt); err != nil {
				return nil, err
			}
			res = append(res, constraintStmt)
		}
	case *dbconn.MySQLConn:
		rows, err := conn.Query(fmt.Sprintf(mysqlQuery, table.Table))
		if err != nil {
			return nil, errors.Wrapf(err, "failed to get the constraints for table %s", table.Table)
		}
		var tableName string
		var createTableStmt string
		for rows.Next() {
			if err := rows.Scan(&tableName, &createTableStmt); err != nil {
				return nil, errors.Wrapf(err, "failed to scan results to get the constraints for table %s", table.Table)
			}
			res = append(res, formatMySQLConstraints(createTableStmt)...)
		}
	}
	return res, nil
}

func formatMySQLConstraints(createTableStmt string) []string {
	var res []string
	const (
		uniqueKeyMySQLRegex = `UNIQUE KEY [^\n]+`
		fkMySQLRegex        = `CONSTRAINT \S+ FOREIGN KEY [^\n]+`
		checkMySQLRegex     = `CONSTRAINT \S+ CHECK [^\n]+`
	)

	for _, rx := range []string{
		uniqueKeyMySQLRegex,
		fkMySQLRegex,
		checkMySQLRegex,
	} {
		ks := regexp.MustCompile(rx).FindAllStringSubmatch(createTableStmt, -1)
		if len(ks) > 0 {
			for _, kgroup := range ks {
				res = append(res, strings.TrimSuffix(kgroup[0], ","))
			}
		}
	}
	return res
}
