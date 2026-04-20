use reqwest::blocking::{Client, Response};

use crate::runner_process_support::HostProcessRunner;

impl HostProcessRunner {
    pub(crate) fn post_json_path(&mut self, path: &str, client: &Client, body: &str) -> Response {
        let url = self.url(path);
        self.post(&url, client, body)
    }
}
