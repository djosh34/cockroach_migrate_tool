use reqwest::blocking::{Client, Response};

use crate::runner_process_support::HostProcessRunner;

impl HostProcessRunner {
    pub(crate) fn metrics_url(&mut self) -> String {
        self.url("/metrics")
    }

    pub(crate) fn ingest_url(&mut self, mapping_id: &str) -> String {
        self.url(&format!("/ingest/{mapping_id}"))
    }

    pub(crate) fn post(&self, url: &str, client: &Client, body: &str) -> Response {
        client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .expect("ingest request should complete")
    }

    pub(crate) fn post_mapping(
        &mut self,
        mapping_id: &str,
        client: &Client,
        body: &str,
    ) -> Response {
        let url = self.ingest_url(mapping_id);
        self.post(&url, client, body)
    }
}
