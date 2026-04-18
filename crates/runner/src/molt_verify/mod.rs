use std::{fs, path::PathBuf, process::Command};

use serde::{Deserialize, Serialize};

use crate::{
    config::{LoadedRunnerConfig, MappingConfig},
    error::{RunnerArtifactError, RunnerVerifyError},
};

pub(crate) fn run_verify(
    loaded_config: &LoadedRunnerConfig,
    mapping_id: &str,
    source_url: &str,
    allow_tls_mode_disable: bool,
) -> Result<MoltVerifySummary, RunnerVerifyError> {
    let request = MoltVerifyRequest::from_loaded_config(
        loaded_config,
        mapping_id,
        source_url,
        allow_tls_mode_disable,
    )?;
    let output = request.execute()?;
    MoltVerifySummary::from_output(request, output)
}

struct MoltVerifyRequest {
    command: String,
    mapping_id: String,
    report_dir: PathBuf,
    source_url: String,
    target_url: String,
    schema_filter: String,
    table_filter: String,
    selected_tables: Vec<String>,
    allow_tls_mode_disable: bool,
}

impl MoltVerifyRequest {
    fn from_loaded_config(
        loaded_config: &LoadedRunnerConfig,
        mapping_id: &str,
        source_url: &str,
        allow_tls_mode_disable: bool,
    ) -> Result<Self, RunnerVerifyError> {
        let mapping = loaded_config.config().mapping(mapping_id).ok_or_else(|| {
            RunnerVerifyError::UnknownMapping {
                mapping_id: mapping_id.to_owned(),
                config_path: loaded_config.path().to_path_buf(),
            }
        })?;
        let selected_tables = mapping.source().tables().to_vec();
        let schema_filter = build_schema_filter(mapping)?;
        let table_filter = build_table_filter(mapping)?;
        let connection = mapping.destination().connection();

        Ok(Self {
            command: loaded_config.config().verify().molt().command().to_owned(),
            mapping_id: mapping.id().to_owned(),
            report_dir: loaded_config.config().verify().molt().report_dir().to_path_buf(),
            source_url: source_url.to_owned(),
            target_url: format!(
                "postgresql://{}:{}@{}:{}/{}",
                connection.user(),
                connection.password(),
                connection.host(),
                connection.port(),
                connection.database()
            ),
            schema_filter,
            table_filter,
            selected_tables,
            allow_tls_mode_disable,
        })
    }

    fn execute(&self) -> Result<MoltVerifyOutput, RunnerVerifyError> {
        let mut command = Command::new(&self.command);
        command
            .arg("verify")
            .arg("--source")
            .arg(&self.source_url)
            .arg("--target")
            .arg(&self.target_url)
            .arg("--schema-filter")
            .arg(&self.schema_filter)
            .arg("--table-filter")
            .arg(&self.table_filter);
        if self.allow_tls_mode_disable {
            command.arg("--allow-tls-mode-disable");
        }

        let output = command
            .output()
            .map_err(|source| RunnerVerifyError::SpawnCommand {
                command: self.command.clone(),
                source,
            })?;
        let status = output.status.code().unwrap_or_default();
        if !output.status.success() {
            return Err(RunnerVerifyError::CommandFailed {
                command: self.command.clone(),
                status,
            });
        }

        Ok(MoltVerifyOutput {
            status,
            raw_output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

fn build_schema_filter(mapping: &MappingConfig) -> Result<String, RunnerVerifyError> {
    let mut schemas = Vec::new();
    for table in mapping.source().tables() {
        let (schema, _) = split_table_name(mapping.id(), table)?;
        if !schemas.iter().any(|existing| existing == schema) {
            schemas.push(schema.to_owned());
        }
    }
    Ok(schemas.join("|"))
}

fn build_table_filter(mapping: &MappingConfig) -> Result<String, RunnerVerifyError> {
    let mut tables = Vec::with_capacity(mapping.source().tables().len());
    for table in mapping.source().tables() {
        let (_, table_name) = split_table_name(mapping.id(), table)?;
        tables.push(table_name.to_owned());
    }
    Ok(tables.join("|"))
}

fn split_table_name<'a>(
    mapping_id: &str,
    table: &'a str,
) -> Result<(&'a str, &'a str), RunnerVerifyError> {
    table
        .split_once('.')
        .ok_or_else(|| RunnerVerifyError::InvalidMappedTable {
            mapping_id: mapping_id.to_owned(),
            table: table.to_owned(),
        })
}

fn parse_json_records(output: &str) -> Vec<MoltLogRecord> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<MoltLogRecord>(line.trim()).ok())
        .collect()
}

struct MoltVerifyOutput {
    status: i32,
    raw_output: String,
}

#[derive(Clone, Debug)]
pub struct MoltVerifySummary {
    mapping_id: String,
    selected_tables: Vec<String>,
    status: i32,
    report_dir: String,
}

impl MoltVerifySummary {
    fn from_output(
        request: MoltVerifyRequest,
        output: MoltVerifyOutput,
    ) -> Result<Self, RunnerVerifyError> {
        write_raw_log(&request, &output)?;
        let records = parse_json_records(&output.raw_output);
        let summaries: Vec<_> = records
            .iter()
            .filter_map(MoltLogRecord::summary)
            .collect();
        if summaries.is_empty() {
            return Err(RunnerVerifyError::MissingSummary {
                mapping_id: request.mapping_id,
            });
        }

        let has_completion = records.iter().any(MoltLogRecord::is_completion);
        if !has_completion {
            return Err(RunnerVerifyError::MissingCompletion {
                mapping_id: request.mapping_id,
            });
        }

        let mismatch_details: Vec<_> = summaries
            .iter()
            .filter(|summary| summary.has_mismatch())
            .map(MoltTableSummary::render_counts)
            .collect();
        let verdict = if mismatch_details.is_empty() {
            "matched"
        } else {
            "mismatch"
        };
        write_artifacts(&request, &output, &summaries, verdict)?;
        if !mismatch_details.is_empty() {
            return Err(RunnerVerifyError::DataMismatch {
                mapping_id: request.mapping_id,
                details: mismatch_details.join(", "),
            });
        }

        Ok(Self {
            mapping_id: request.mapping_id,
            selected_tables: request.selected_tables,
            status: output.status,
            report_dir: request.report_dir.display().to_string(),
        })
    }
}

fn write_artifacts(
    request: &MoltVerifyRequest,
    output: &MoltVerifyOutput,
    summaries: &[MoltTableSummary],
    verdict: &str,
) -> Result<(), RunnerVerifyError> {
    let report_dir = ensure_report_dir(request)?;
    let summary_path = report_dir.join(format!("{}.summary.json", request.mapping_id));
    let summary = ArtifactSummary {
        mapping_id: request.mapping_id.clone(),
        process_exit: output.status,
        verdict: verdict.to_owned(),
        selected_tables: request.selected_tables.clone(),
        table_summaries: summaries.to_vec(),
    };
    let summary_json = serde_json::to_string_pretty(&summary)
        .expect("verification summary JSON serialization should succeed");
    fs::write(&summary_path, format!("{summary_json}\n")).map_err(|source| {
        RunnerArtifactError::WriteFile {
            path: summary_path,
            source,
        }
    })?;

    Ok(())
}

fn write_raw_log(
    request: &MoltVerifyRequest,
    output: &MoltVerifyOutput,
) -> Result<PathBuf, RunnerVerifyError> {
    let report_dir = ensure_report_dir(request)?;
    let raw_log_path = report_dir.join(format!("{}.raw.log", request.mapping_id));
    fs::write(&raw_log_path, &output.raw_output).map_err(|source| RunnerArtifactError::WriteFile {
        path: raw_log_path.clone(),
        source,
    })?;
    Ok(raw_log_path)
}

fn ensure_report_dir(request: &MoltVerifyRequest) -> Result<PathBuf, RunnerVerifyError> {
    let report_dir = request.report_dir.clone();
    fs::create_dir_all(&report_dir).map_err(|source| RunnerArtifactError::CreateOutputDirectory {
        path: report_dir.clone(),
        source,
    })?;
    Ok(report_dir)
}

#[derive(Serialize)]
struct ArtifactSummary {
    mapping_id: String,
    process_exit: i32,
    verdict: String,
    selected_tables: Vec<String>,
    table_summaries: Vec<MoltTableSummary>,
}

impl std::fmt::Display for MoltVerifySummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "verification mapping={} tables={} process_exit={} verdict=matched artifacts={}",
            self.mapping_id,
            self.selected_tables.join(","),
            self.status,
            self.report_dir
        )
    }
}

#[derive(Debug, Deserialize)]
struct MoltLogRecord {
    #[serde(rename = "type")]
    record_type: Option<String>,
    table_schema: Option<String>,
    table_name: Option<String>,
    message: Option<String>,
    num_missing: Option<u64>,
    num_mismatch: Option<u64>,
    num_extraneous: Option<u64>,
    num_column_mismatch: Option<u64>,
}

impl MoltLogRecord {
    fn summary(&self) -> Option<MoltTableSummary> {
        if self.record_type.as_deref() != Some("summary") {
            return None;
        }

        Some(MoltTableSummary {
            table_name: format!(
                "{}.{}",
                self.table_schema.as_deref()?,
                self.table_name.as_deref()?
            ),
            num_missing: self.num_missing?,
            num_mismatch: self.num_mismatch?,
            num_extraneous: self.num_extraneous?,
            num_column_mismatch: self.num_column_mismatch?,
        })
    }

    fn is_completion(&self) -> bool {
        self.message.as_deref() == Some("verification complete")
    }
}

#[derive(Clone, Serialize)]
struct MoltTableSummary {
    table_name: String,
    num_missing: u64,
    num_mismatch: u64,
    num_extraneous: u64,
    num_column_mismatch: u64,
}

impl MoltTableSummary {
    fn has_mismatch(&self) -> bool {
        self.num_missing > 0
            || self.num_mismatch > 0
            || self.num_extraneous > 0
            || self.num_column_mismatch > 0
    }

    fn render_counts(&self) -> String {
        format!(
            "{}(num_missing={}, num_mismatch={}, num_extraneous={}, num_column_mismatch={})",
            self.table_name,
            self.num_missing,
            self.num_mismatch,
            self.num_extraneous,
            self.num_column_mismatch
        )
    }
}
