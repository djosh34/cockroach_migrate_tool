use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
};

use reqwest::blocking::Client;
use serde_json::Value;

pub(crate) struct HostProcessRunner {
    pub(crate) child: Child,
    webhook_base_url: Option<String>,
}

impl HostProcessRunner {
    pub(crate) fn start(config_path: &Path) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_runner"))
            .args(["run", "--log-format", "json", "--config"])
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("runner should start");
        Self {
            child,
            webhook_base_url: None,
        }
    }

    pub(crate) fn read_stderr_event(&mut self) -> Value {
        let log_line = self.read_stderr_line();
        serde_json::from_str(log_line.trim()).expect("runner log should be valid json")
    }

    pub(crate) fn healthz_url(&mut self) -> String {
        self.url("/healthz")
    }

    pub(crate) fn url(&mut self, path: &str) -> String {
        format!("{}{path}", self.webhook_base_url())
    }

    pub(crate) fn assert_healthy(&mut self, client: &Client) {
        let url = self.healthz_url();
        for _ in 0..50 {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("runner child status should be readable")
            {
                let (stdout, stderr) = self.read_logs();
                panic!(
                    "runner exited before serving healthz with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                );
            }

            match client.get(&url).send() {
                Ok(response) if response.status().is_success() => {
                    let body = response.text().expect("healthz body should be readable");
                    assert_eq!(body, "ok");
                    return;
                }
                Ok(_) | Err(_) => std::thread::sleep(std::time::Duration::from_millis(100)),
            }
        }

        panic!("runner did not serve healthz at {url}");
    }

    fn webhook_base_url(&mut self) -> &str {
        if self.webhook_base_url.is_none() {
            self.webhook_base_url = Some(self.discover_webhook_base_url());
        }
        self.webhook_base_url
            .as_deref()
            .expect("webhook base url should be cached")
    }

    fn discover_webhook_base_url(&mut self) -> String {
        for _ in 0..10 {
            let payload = self.read_stderr_event();
            let json_object = payload
                .as_object()
                .expect("runner log should be a json object");

            match json_object.get("event").and_then(Value::as_str) {
                Some("runtime.starting") => continue,
                Some("webhook.bound") => {
                    let bind_addr = json_object
                        .get("webhook")
                        .and_then(Value::as_str)
                        .expect("webhook bound event must expose a webhook address");
                    let scheme = json_object
                        .get("mode")
                        .and_then(Value::as_str)
                        .expect("webhook bound event must expose a webhook mode");
                    let port = bind_addr
                        .rsplit(':')
                        .next()
                        .expect("webhook bound address must include a port");
                    return format!("{scheme}://localhost:{port}");
                }
                Some(other) => panic!("unexpected runner startup event `{other}`"),
                None => panic!("runner startup log must include a string event field"),
            }
        }

        panic!("runner did not report a bound webhook address during startup");
    }

    pub(crate) fn read_logs(&mut self) -> (String, String) {
        let mut stdout = String::new();
        let mut stderr = String::new();
        self.child
            .stdout
            .as_mut()
            .expect("runner stdout pipe should exist")
            .read_to_string(&mut stdout)
            .expect("runner stdout should be readable");
        self.child
            .stderr
            .as_mut()
            .expect("runner stderr pipe should exist")
            .read_to_string(&mut stderr)
            .expect("runner stderr should be readable");
        (stdout, stderr)
    }

    fn read_stderr_line(&mut self) -> String {
        let stderr = self
            .child
            .stderr
            .as_mut()
            .expect("runner stderr pipe should exist");
        let mut bytes = Vec::new();

        loop {
            let mut byte = [0_u8; 1];
            let read = stderr
                .read(&mut byte)
                .expect("runner stderr should be readable");
            assert_ne!(read, 0, "runner stderr closed before emitting a log line");
            bytes.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }

        String::from_utf8(bytes).expect("runner stderr line should be utf-8")
    }
}

impl Drop for HostProcessRunner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
