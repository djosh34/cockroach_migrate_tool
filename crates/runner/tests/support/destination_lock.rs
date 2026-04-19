use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

pub(crate) struct DestinationTableLock {
    child: Child,
    postgres_host_port: u16,
    database: String,
    schema_name: String,
    table_name: String,
    application_name: String,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl DestinationTableLock {
    pub(crate) fn acquire(
        postgres_host_port: u16,
        database: &str,
        qualified_table: &str,
        application_name: &str,
        stdout_path: &Path,
        stderr_path: &Path,
    ) -> Self {
        let (schema_name, table_name) = split_table_reference(qualified_table);
        let stdout = File::create(stdout_path).expect("destination lock stdout log should open");
        let stderr = File::create(stderr_path).expect("destination lock stderr log should open");
        let child = Command::new("psql")
            .env("PGPASSWORD", "postgres")
            .env("PGAPPNAME", application_name)
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &postgres_host_port.to_string(),
                "-U",
                "postgres",
                "-d",
                database,
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                &format!(
                    "BEGIN; LOCK TABLE {qualified_table} IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(600);"
                ),
            ])
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("destination lock process should start");
        let mut lock = Self {
            child,
            postgres_host_port,
            database: database.to_owned(),
            schema_name: schema_name.to_owned(),
            table_name: table_name.to_owned(),
            application_name: application_name.to_owned(),
            stdout_path: stdout_path.to_path_buf(),
            stderr_path: stderr_path.to_path_buf(),
        };
        lock.wait_until_acquired();
        lock
    }

    fn wait_until_acquired(&mut self) {
        for _ in 0..60 {
            self.assert_alive();
            if self.access_exclusive_lock_is_held() {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "destination lock was not acquired for {}.{}\nstdout:\n{}\nstderr:\n{}",
            self.schema_name,
            self.table_name,
            read_file(&self.stdout_path),
            read_file(&self.stderr_path),
        );
    }

    fn access_exclusive_lock_is_held(&self) -> bool {
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
                "-t",
                "-A",
                "-c",
                &format!(
                    "SELECT EXISTS (
                         SELECT 1
                         FROM pg_locks lock
                         JOIN pg_stat_activity activity ON activity.pid = lock.pid
                         JOIN pg_class relation ON relation.oid = lock.relation
                         JOIN pg_namespace namespace ON namespace.oid = relation.relnamespace
                         WHERE lock.granted
                           AND lock.mode = 'AccessExclusiveLock'
                           AND activity.application_name = '{application_name}'
                           AND namespace.nspname = '{schema_name}'
                           AND relation.relname = '{table_name}'
                     );",
                    application_name = self.application_name,
                    schema_name = self.schema_name,
                    table_name = self.table_name,
                ),
            ])
            .output()
            .unwrap_or_else(|error| panic!("psql should start for lock probe: {error}"));
        assert!(
            output.status.success(),
            "lock probe failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8(output.stdout)
            .expect("lock probe stdout should be utf-8")
            .trim()
            == "t"
    }

    fn assert_alive(&mut self) {
        if let Some(status) = self
            .child
            .try_wait()
            .expect("destination lock process status should be readable")
        {
            panic!(
                "destination lock process exited early with status {status}\nstdout:\n{}\nstderr:\n{}",
                read_file(&self.stdout_path),
                read_file(&self.stderr_path),
            );
        }
    }
}

impl Drop for DestinationTableLock {
    fn drop(&mut self) {
        let child_is_still_running = self
            .child
            .try_wait()
            .expect("destination lock process status should be readable on drop")
            .is_none();
        if child_is_still_running {
            self.child
                .kill()
                .expect("destination lock process should be killable on drop");
        }
        self.terminate_backend_session();
        if child_is_still_running {
            self.child
                .wait()
                .expect("destination lock process should be waitable on drop");
        }
    }
}

fn read_file(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("failed to read `{}`: {error}", path.display()),
    }
}

fn split_table_reference(table: &str) -> (&str, &str) {
    table
        .split_once('.')
        .unwrap_or_else(|| panic!("table should be qualified as schema.table: {table}"))
}

impl DestinationTableLock {
    fn terminate_backend_session(&self) {
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
                "-t",
                "-A",
                "-c",
                &format!(
                    "SELECT count(*)::text
                     FROM (
                         SELECT pg_terminate_backend(pid)
                         FROM pg_stat_activity
                         WHERE application_name = '{application_name}'
                     ) terminated;",
                    application_name = self.application_name,
                ),
            ])
            .output()
            .unwrap_or_else(|error| panic!("psql should start for lock termination: {error}"));
        assert!(
            output.status.success(),
            "lock termination failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
