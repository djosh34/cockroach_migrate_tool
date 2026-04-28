use std::{ffi::OsStr, path::PathBuf, process::Output};

use crate::predicates::Predicate;

pub struct Command {
    inner: std::process::Command,
}

impl Command {
    pub fn cargo_bin(name: &str) -> Result<Self, String> {
        let env_var = format!("CARGO_BIN_EXE_{}", name.replace('-', "_"));
        let binary_path = std::env::var_os(&env_var)
            .map(PathBuf::from)
            .or_else(|| fallback_binary_path(name))
            .ok_or_else(|| {
                format!("cargo binary `{name}` should exist via `{env_var}` or next to the test executable")
            })?;

        Ok(Self {
            inner: std::process::Command::new(binary_path),
        })
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.inner.args(args);
        self
    }

    pub fn arg<S>(&mut self, arg: S) -> &mut Self
    where
        S: AsRef<OsStr>,
    {
        self.inner.arg(arg);
        self
    }

    pub fn assert(&mut self) -> Assert {
        let output = self
            .inner
            .output()
            .expect("command should run for assertion");
        Assert { output }
    }
}

pub struct Assert {
    output: Output,
}

impl Assert {
    pub fn success(self) -> Self {
        assert!(
            self.output.status.success(),
            "command unexpectedly failed with status {}\nstdout:\n{}\nstderr:\n{}",
            self.output.status,
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub fn failure(self) -> Self {
        assert!(
            !self.output.status.success(),
            "command unexpectedly succeeded with status {}\nstdout:\n{}\nstderr:\n{}",
            self.output.status,
            String::from_utf8_lossy(&self.output.stdout),
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub fn stdout(self, predicate: impl Predicate<str>) -> Self {
        let stdout = String::from_utf8_lossy(&self.output.stdout);
        assert!(
            predicate.eval(stdout.as_ref()),
            "stdout did not match expectation\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            self.output.status,
            stdout,
            String::from_utf8_lossy(&self.output.stderr),
        );
        self
    }

    pub fn stderr(self, predicate: impl Predicate<str>) -> Self {
        let stderr = String::from_utf8_lossy(&self.output.stderr);
        assert!(
            predicate.eval(stderr.as_ref()),
            "stderr did not match expectation\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            self.output.status,
            String::from_utf8_lossy(&self.output.stdout),
            stderr,
        );
        self
    }

    pub fn get_output(&self) -> &Output {
        &self.output
    }
}

fn fallback_binary_path(name: &str) -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let target_dir = current_exe.parent()?.parent()?;
    let binary_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let candidate = target_dir.join(binary_name);

    candidate.is_file().then_some(candidate)
}
