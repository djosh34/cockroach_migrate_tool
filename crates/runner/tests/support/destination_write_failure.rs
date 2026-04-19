use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    process::Command,
};

pub(crate) struct DestinationWriteFailure {
    postgres_host_port: u16,
    database: String,
    schema_name: String,
    table_name: String,
    function_name: String,
    trigger_name: String,
    installed: bool,
}

pub(crate) struct DestinationWriteFailureSpec<'a> {
    pub(crate) schema_name: &'a str,
    pub(crate) table_name: &'a str,
    pub(crate) row_predicate_sql: &'a str,
    pub(crate) error_message: &'a str,
}

impl DestinationWriteFailure {
    pub(crate) fn install(
        postgres_host_port: u16,
        database: &str,
        spec: DestinationWriteFailureSpec<'_>,
    ) -> Self {
        let mut failure = Self {
            postgres_host_port,
            database: database.to_owned(),
            schema_name: spec.schema_name.to_owned(),
            table_name: spec.table_name.to_owned(),
            function_name: scoped_identifier("fail_write_fn", spec.schema_name, spec.table_name),
            trigger_name: scoped_identifier("fail_write_trg", spec.schema_name, spec.table_name),
            installed: false,
        };
        failure.create(spec.row_predicate_sql, spec.error_message);
        failure.installed = true;
        failure
    }

    fn create(&self, row_predicate_sql: &str, error_message: &str) {
        self.exec_psql(&format!(
            r#"
DROP TRIGGER IF EXISTS "{trigger_name}" ON "{schema_name}"."{table_name}";
DROP FUNCTION IF EXISTS "{schema_name}"."{function_name}"();
CREATE FUNCTION "{schema_name}"."{function_name}"() RETURNS trigger
LANGUAGE plpgsql
AS $function$
BEGIN
    IF {row_predicate_sql} THEN
        RAISE EXCEPTION '{error_message}';
    END IF;
    RETURN NEW;
END;
$function$;
CREATE TRIGGER "{trigger_name}"
BEFORE INSERT OR UPDATE ON "{schema_name}"."{table_name}"
FOR EACH ROW
EXECUTE FUNCTION "{schema_name}"."{function_name}"();
"#,
            trigger_name = self.trigger_name,
            schema_name = self.schema_name,
            table_name = self.table_name,
            function_name = self.function_name,
            row_predicate_sql = row_predicate_sql,
            error_message = error_message.replace('\'', "''"),
        ));
    }

    fn drop_trigger_and_function(&self) {
        self.exec_psql(&format!(
            r#"
DROP TRIGGER IF EXISTS "{trigger_name}" ON "{schema_name}"."{table_name}";
DROP FUNCTION IF EXISTS "{schema_name}"."{function_name}"();
"#,
            trigger_name = self.trigger_name,
            schema_name = self.schema_name,
            table_name = self.table_name,
            function_name = self.function_name,
        ));
    }

    fn exec_psql(&self, sql: &str) {
        let output = Command::new("psql")
            .env("PGPASSWORD", "postgres")
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &self.postgres_host_port.to_string(),
                "-U",
                "postgres",
                "-d",
                &self.database,
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("psql should start for destination write failure: {error}")
            });
        assert!(
            output.status.success(),
            "destination write failure psql failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

impl Drop for DestinationWriteFailure {
    fn drop(&mut self) {
        if self.installed {
            self.drop_trigger_and_function();
        }
    }
}

fn scoped_identifier(prefix: &str, schema_name: &str, table_name: &str) -> String {
    let mut hasher = DefaultHasher::new();
    schema_name.hash(&mut hasher);
    table_name.hash(&mut hasher);
    format!("{prefix}_{:016x}", hasher.finish())
}
