use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

pub(crate) struct RunnerProcess {
    child: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl RunnerProcess {
    pub(crate) fn start(config_path: &Path, stdout_path: &Path, stderr_path: &Path) -> Self {
        let stdout = File::create(stdout_path).expect("runner stdout log should open");
        let stderr = File::create(stderr_path).expect("runner stderr log should open");
        let child = Command::new(env!("CARGO_BIN_EXE_runner"))
            .args(["run", "--config"])
            .arg(config_path)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("runner process should start");
        Self {
            child,
            stdout_path: stdout_path.to_path_buf(),
            stderr_path: stderr_path.to_path_buf(),
        }
    }

    pub(crate) fn assert_alive(&mut self) {
        if let Some(status) = self
            .child
            .try_wait()
            .expect("runner process status should be readable")
        {
            panic!(
                "runner exited early with status {status}\nstdout:\n{}\nstderr:\n{}",
                read_file(&self.stdout_path),
                read_file(&self.stderr_path),
            );
        }
    }

    pub(crate) fn kill(&mut self) {
        self.child
            .kill()
            .expect("runner process should be killable");
        self.child
            .wait()
            .expect("runner process should be waitable after kill");
    }

    pub(crate) fn wait_for_failed_exit(&mut self) -> String {
        for _ in 0..120 {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("runner process status should be readable")
            {
                let (stdout, stderr) = self.read_logs();
                assert!(
                    !status.success(),
                    "runner exited successfully but failure was expected\nstdout:\n{stdout}\nstderr:\n{stderr}"
                );
                return stderr;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }

        let (stdout, stderr) = self.read_logs();
        panic!("runner did not exit with failure in time\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    fn read_logs(&self) -> (String, String) {
        (read_file(&self.stdout_path), read_file(&self.stderr_path))
    }
}

impl Drop for RunnerProcess {
    fn drop(&mut self) {
        if self
            .child
            .try_wait()
            .expect("runner process status should be readable on drop")
            .is_none()
        {
            self.child
                .kill()
                .expect("runner process should be killable on drop");
            self.child
                .wait()
                .expect("runner process should be waitable on drop");
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
