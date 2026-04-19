package dbtable

import (
	"fmt"
	"strings"

	"github.com/cockroachdb/cockroachdb-parser/pkg/sql/sem/tree"
	"github.com/lib/pq/oid"
)

type Name struct {
	Schema tree.Name
	Table  tree.Name
}

func (n Name) MakeTableName() tree.TableName {
	return tree.MakeTableNameFromPrefix(tree.ObjectNamePrefix{
		SchemaName:     n.Schema,
		ExplicitSchema: true,
	}, n.Table)
}

func (n Name) NewTableName() *tree.TableName {
	tn := n.MakeTableName()
	return &tn
}

// DBTable represents a basic table object with OID from the relevant table.
type DBTable struct {
	Name
	OID          oid.Oid
	LocalityExpr string
}

func (n Name) SafeString() string {
	return fmt.Sprintf("%s.%s", n.Schema, n.Table)
}

func (tm DBTable) Compare(o DBTable) int {
	if c := strings.Compare(strings.ToLower(string(tm.Schema)), strings.ToLower(string(o.Schema))); c != 0 {
		return c
	}
	return strings.Compare(strings.ToLower(string(tm.Table)), strings.ToLower(string(o.Table)))
}

func (tm DBTable) Less(o DBTable) bool {
	return tm.Compare(o) < 0
}

func (tm DBTable) String() string {
	return fmt.Sprintf("%s.%s", tm.Schema, tm.Table)
}

// VerifiedTable represents a table which has been verified across implementations.
type VerifiedTable struct {
	Name
	PrimaryKeyColumns []tree.Name
	Columns           ColumnListWithAttr
	ColumnOIDs        [2][]oid.Oid
}

type ColumnListWithAttr []*ColumnWithAttr

func (cl ColumnListWithAttr) ColumnNames() []tree.Name {
	res := make([]tree.Name, 0, len(cl))
	for _, c := range cl {
		res = append(res, c.Name)
	}
	return res
}

func (cl ColumnListWithAttr) NotNullColIdxMapping() map[int]bool {
	res := make(map[int]bool)
	for i, c := range cl {
		if c.NotNull {
			res[i] = true
		}
	}
	return res
}

type ColumnWithAttr struct {
	Name    tree.Name
	NotNull bool
}
