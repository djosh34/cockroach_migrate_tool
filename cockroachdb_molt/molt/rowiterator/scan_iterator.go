package rowiterator

import (
	"context"
	"fmt"
	"go/constant"
	"time"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree/treecmp"
	"github.com/cockroachdb/errors"
	"github.com/cockroachdb/molt/dbconn"
	"golang.org/x/time/rate"
)

type scanIterator struct {
	conn         dbconn.Conn
	table        ScanTable
	rowBatchSize int

	waitCh        chan scanIteratorResult
	cache         []tree.Datums
	pkCursor      tree.Datums
	currCacheSize int
	err           error
	scanQuery     scanQuery
	rateLimiter   *rate.Limiter
}

type scanIteratorResult struct {
	r   []tree.Datums
	err error
}

// NewScanIterator returns a row iterator which scans the given table.
func NewScanIterator(
	ctx context.Context,
	conn dbconn.Conn,
	table ScanTable,
	rowBatchSize int,
	rateLimiter *rate.Limiter,
) (Iterator, error) {
	// Initialize the type map on the connection.
	for _, typOID := range table.ColumnOIDs {
		if _, err := dbconn.GetDataType(ctx, conn, typOID); err != nil {
			return nil, errors.Wrapf(err, "Error initializing type oid %d", typOID)
		}
	}
	it := &scanIterator{
		conn:          conn,
		table:         table,
		rowBatchSize:  rowBatchSize,
		currCacheSize: rowBatchSize,
		waitCh:        make(chan scanIteratorResult, 1),
		rateLimiter:   rateLimiter,
	}
	switch conn := conn.(type) {
	case *dbconn.PGConn:
		it.scanQuery = newPGScanQuery(table, rowBatchSize)
	default:
		return nil, errors.Newf("unsupported conn type %T", conn)
	}
	it.nextPage(ctx)
	return it, nil
}

func (it *scanIterator) Conn() dbconn.Conn {
	return it.conn
}

func (it *scanIterator) HasNext(ctx context.Context) bool {
	for {
		if it.err != nil {
			return false
		}

		if len(it.cache) > 0 {
			return true
		}

		// If the last cache size was less than the row size, we're done
		// reading all the results.
		if it.currCacheSize < it.rowBatchSize {
			return false
		}

		// Wait for more results.
		res := <-it.waitCh
		if res.err != nil {
			it.err = errors.Wrap(res.err, "error getting result")
			return false
		}
		it.cache = res.r
		it.currCacheSize = len(it.cache)

		// Queue the next page immediately.
		if it.currCacheSize == it.rowBatchSize {
			it.nextPage(ctx)
		}
	}
}

// nextPage fetches rows asynchronously.
func (it *scanIterator) nextPage(ctx context.Context) {
	go func() {
		datums, err := func() ([]tree.Datums, error) {
			var currRows rows
			q, args, err := it.scanQuery.generate(it.pkCursor)
			if err != nil {
				return nil, err
			}
			if it.rateLimiter != nil {
				if err := it.rateLimiter.Wait(ctx); err != nil {
					return nil, err
				}
			}
			switch conn := it.conn.(type) {
			case *dbconn.PGConn:
				newRows, err := conn.Query(ctx, q, args...)
				if err != nil {
					fmt.Printf("schema:%s, table:%s\n", it.table.Schema, it.table.Table.Name)
					return nil, errors.Wrapf(err, "[pg]error getting rows for table %s.%s from %s", it.table.Schema, it.table.Table.Name, it.conn.ID)
				}
				currRows = &pgRows{
					Rows:    newRows,
					typMap:  it.conn.TypeMap(),
					typOIDs: it.table.ColumnOIDs,
				}
			default:
				return nil, errors.AssertionFailedf("unhandled conn type: %T", conn)
			}
			defer func() { currRows.Close() }()

			datums := make([]tree.Datums, 0, it.rowBatchSize)
			for currRows.Next() {
				d, err := currRows.Datums()
				if err != nil {
					return nil, errors.Wrapf(err, "error getting datums")
				}
				it.pkCursor = d[:len(it.table.PrimaryKeyColumns)]
				datums = append(datums, d)
			}
			return datums, currRows.Err()
		}()
		it.waitCh <- scanIteratorResult{r: datums, err: err}
	}()
}

func (it *scanIterator) Peek(ctx context.Context) tree.Datums {
	if it.HasNext(ctx) {
		return it.cache[0]
	}
	return nil
}

func (it *scanIterator) Next(ctx context.Context) tree.Datums {
	if it.HasNext(ctx) {
		ret := it.cache[0]
		it.cache = it.cache[1:]
		return ret
	}
	return nil
}

func (it *scanIterator) Error() error {
	return it.err
}

type scanQuery struct {
	base  any
	table ScanTable
}

func newPGScanQuery(table ScanTable, rowBatchSize int) scanQuery {
	baseSelectExpr := NewPGBaseSelectClause(table.Table)
	baseSelectExpr.Limit = &tree.Limit{Count: tree.NewNumVal(constant.MakeUint64(uint64(rowBatchSize)), "", false)}
	if table.AOST != nil {
		var err error
		baseSelectExpr.Select.(*tree.SelectClause).From.AsOf.Expr, err = tree.MakeDTimestamp(*table.AOST, time.Microsecond)
		if err != nil {
			panic(err)
		}
	}
	return scanQuery{
		base:  baseSelectExpr,
		table: table,
	}
}

func NewPGBaseSelectClause(table Table) *tree.Select {
	tn := tree.MakeTableNameFromPrefix(
		tree.ObjectNamePrefix{SchemaName: table.Schema, ExplicitSchema: true},
		table.Table,
	)
	selectClause := &tree.SelectClause{
		From: tree.From{
			Tables: tree.TableExprs{&tn},
		},
	}
	for _, col := range table.ColumnsWithAttr {
		selectClause.Exprs = append(
			selectClause.Exprs,
			tree.SelectExpr{
				Expr: tree.NewUnresolvedName(string(col.Name)),
			},
		)
	}
	baseSelectExpr := &tree.Select{
		Select: selectClause,
	}
	for _, pkCol := range table.PrimaryKeyColumns {
		baseSelectExpr.OrderBy = append(
			baseSelectExpr.OrderBy,
			&tree.Order{Expr: tree.NewUnresolvedName(string(pkCol))},
		)
	}
	return baseSelectExpr
}

func (sq *scanQuery) generate(pkCursor tree.Datums) (string, []any, error) {
	switch stmt := sq.base.(type) {
	case *tree.Select:
		andClause := &tree.AndExpr{
			Left:  tree.DBoolTrue,
			Right: tree.DBoolTrue,
		}
		// Use the cursor if available, otherwise not.
		if len(pkCursor) > 0 {
			andClause.Left = makePGCompareExpr(
				treecmp.MakeComparisonOperator(treecmp.GT),
				sq.table.ColumnsWithAttr.ColumnNames(),
				pkCursor,
			)
		} else if len(sq.table.StartPKVals) > 0 {
			andClause.Left = makePGCompareExpr(
				treecmp.MakeComparisonOperator(treecmp.GE),
				sq.table.ColumnsWithAttr.ColumnNames(),
				sq.table.StartPKVals,
			)
		}
		if len(sq.table.EndPKVals) > 0 {
			andClause.Right = makePGCompareExpr(
				treecmp.MakeComparisonOperator(treecmp.LT),
				sq.table.ColumnsWithAttr.ColumnNames(),
				sq.table.EndPKVals,
			)
		}
		stmt.Select.(*tree.SelectClause).Where = &tree.Where{
			Type: tree.AstWhere,
			Expr: andClause,
		}
		f := tree.NewFmtCtx(tree.FmtParsableNumerics)
		f.FormatNode(stmt)
		return f.CloseAndGetString(), nil, nil
	}
	return "", nil, errors.AssertionFailedf("unknown scan query type: %T", sq.base)
}

func makePGCompareExpr(
	op treecmp.ComparisonOperator, cols []tree.Name, vals tree.Datums,
) *tree.ComparisonExpr {
	cmpExpr := &tree.ComparisonExpr{
		Operator: op,
	}
	if len(vals) > 1 {
		colNames := &tree.Tuple{}
		colVals := &tree.Tuple{}
		for i := range vals {
			colNames.Exprs = append(colNames.Exprs, tree.NewUnresolvedName(string(cols[i])))
			colVals.Exprs = append(colVals.Exprs, vals[i])
		}
		cmpExpr.Left = colNames
		cmpExpr.Right = colVals
	} else {
		cmpExpr.Left = tree.NewUnresolvedName(string(cols[0]))
		cmpExpr.Right = vals[0]
	}
	return cmpExpr
}
