use std::fmt::{self, Display, Formatter};

use thiserror::Error;

#[derive(Debug, Error)]
#[error("{report}")]
pub struct SchemaMismatchError {
    report: SchemaMismatchReport,
}

impl SchemaMismatchError {
    pub(crate) fn new(mismatches: Vec<SchemaMismatch>) -> Self {
        Self {
            report: SchemaMismatchReport { mismatches },
        }
    }
}

#[derive(Debug)]
struct SchemaMismatchReport {
    mismatches: Vec<SchemaMismatch>,
}

impl Display for SchemaMismatchReport {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "schema compare mismatch:")?;
        for mismatch in &self.mismatches {
            writeln!(f, "- {mismatch}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum SchemaMismatch {
    MissingTable {
        side: SchemaSide,
        table: String,
    },
    MissingColumn {
        side: SchemaSide,
        table: String,
        column: String,
    },
    UnsupportedTypePair {
        table: String,
        column: String,
        cockroach_type: String,
        postgres_type: String,
    },
    ColumnTypeMismatch {
        table: String,
        column: String,
        cockroach_type: String,
        postgres_type: String,
    },
    NullabilityMismatch {
        table: String,
        column: String,
        cockroach_nullable: bool,
        postgres_nullable: bool,
    },
    PrimaryKeyMismatch {
        table: String,
        cockroach_columns: Vec<String>,
        postgres_columns: Vec<String>,
    },
    UniqueConstraintMismatch {
        table: String,
        cockroach_constraints: Vec<Vec<String>>,
        postgres_constraints: Vec<Vec<String>>,
    },
    ForeignKeyMismatch {
        table: String,
        cockroach_foreign_keys: Vec<String>,
        postgres_foreign_keys: Vec<String>,
    },
    IndexMismatch {
        table: String,
        cockroach_indexes: Vec<String>,
        postgres_indexes: Vec<String>,
    },
}

impl Display for SchemaMismatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTable { side, table } => {
                write!(f, "missing table on {side}: {table}")
            }
            Self::MissingColumn {
                side,
                table,
                column,
            } => write!(f, "missing column on {side}: {table}.{column}"),
            Self::UnsupportedTypePair {
                table,
                column,
                cockroach_type,
                postgres_type,
            } => write!(
                f,
                "unsupported type pair for {table}.{column}: cockroach=`{cockroach_type}` postgres=`{postgres_type}`"
            ),
            Self::ColumnTypeMismatch {
                table,
                column,
                cockroach_type,
                postgres_type,
            } => write!(
                f,
                "type mismatch for {table}.{column}: cockroach=`{cockroach_type}` postgres=`{postgres_type}`"
            ),
            Self::NullabilityMismatch {
                table,
                column,
                cockroach_nullable,
                postgres_nullable,
            } => write!(
                f,
                "nullability mismatch for {table}.{column}: cockroach={} postgres={}",
                render_nullable(*cockroach_nullable),
                render_nullable(*postgres_nullable)
            ),
            Self::PrimaryKeyMismatch {
                table,
                cockroach_columns,
                postgres_columns,
            } => write!(
                f,
                "primary key mismatch for {table}: cockroach=({}) postgres=({})",
                cockroach_columns.join(", "),
                postgres_columns.join(", ")
            ),
            Self::UniqueConstraintMismatch {
                table,
                cockroach_constraints,
                postgres_constraints,
            } => write!(
                f,
                "unique constraint mismatch for {table}: cockroach={} postgres={}",
                render_nested_columns(cockroach_constraints),
                render_nested_columns(postgres_constraints)
            ),
            Self::ForeignKeyMismatch {
                table,
                cockroach_foreign_keys,
                postgres_foreign_keys,
            } => write!(
                f,
                "foreign key mismatch for {table}: cockroach={} postgres={}",
                render_structures(cockroach_foreign_keys),
                render_structures(postgres_foreign_keys)
            ),
            Self::IndexMismatch {
                table,
                cockroach_indexes,
                postgres_indexes,
            } => write!(
                f,
                "index mismatch for {table}: cockroach={} postgres={}",
                render_structures(cockroach_indexes),
                render_structures(postgres_indexes)
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SchemaSide {
    Cockroach,
    Postgres,
}

impl Display for SchemaSide {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cockroach => write!(f, "cockroach"),
            Self::Postgres => write!(f, "postgres"),
        }
    }
}

fn render_nullable(value: bool) -> &'static str {
    if value { "nullable" } else { "not-null" }
}

fn render_nested_columns(value: &[Vec<String>]) -> String {
    if value.is_empty() {
        "none".to_owned()
    } else {
        value.iter()
            .map(|columns| format!("({})", columns.join(", ")))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn render_structures(value: &[String]) -> String {
    if value.is_empty() {
        "none".to_owned()
    } else {
        value.join(", ")
    }
}
