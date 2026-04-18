use std::fmt;

pub struct MappingIngestPath<'a> {
    mapping_id: &'a str,
}

impl<'a> MappingIngestPath<'a> {
    pub fn new(mapping_id: &'a str) -> Self {
        Self { mapping_id }
    }

    pub fn to_url(&self, base_url: &str) -> String {
        format!("{}{}", base_url.trim_end_matches('/'), self)
    }
}

impl fmt::Display for MappingIngestPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/ingest/{}", self.mapping_id)
    }
}

pub fn render_mapping_ingest_url(base_url: &str, mapping_id: &str) -> String {
    MappingIngestPath::new(mapping_id).to_url(base_url)
}
