use operator_log::LogEvent;

use crate::{
    LoadedRunnerConfig, RunnerStartupPlan, RunnerValidateConfigError, validate_destination_group,
};

pub async fn validate_loaded_config(
    loaded_config: &LoadedRunnerConfig,
    deep: bool,
) -> Result<ValidatedConfig, RunnerValidateConfigError> {
    let deep_validation = if deep {
        let startup_plan = RunnerStartupPlan::from_config(loaded_config.config())?;
        for destination_group in startup_plan.destination_groups() {
            validate_destination_group(destination_group).await?;
        }
        DeepValidationStatus::Ok
    } else {
        DeepValidationStatus::Skipped
    };

    Ok(ValidatedConfig::from_loaded_config(
        loaded_config,
        deep_validation,
    ))
}

pub struct ValidatedConfig {
    config_path: String,
    mappings: usize,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_mode: &'static str,
    webhook_tls_files: Option<String>,
    deep_validation: DeepValidationStatus,
}

#[derive(Clone, Copy)]
enum DeepValidationStatus {
    Skipped,
    Ok,
}

impl ValidatedConfig {
    fn from_loaded_config(
        loaded_config: &LoadedRunnerConfig,
        deep_validation: DeepValidationStatus,
    ) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_mode: config.webhook().effective_mode(),
            webhook_tls_files: config.webhook().tls().map(|tls| tls.material_label()),
            deep_validation,
        }
    }

    pub fn text_output(&self) -> String {
        let mut summary = format!(
            "config valid: config={} mappings={} webhook={} mode={}",
            self.config_path, self.mappings, self.webhook_bind_addr, self.webhook_mode
        );
        if let Some(tls) = &self.webhook_tls_files {
            summary.push_str(" tls=");
            summary.push_str(tls);
        }
        if let Some(deep_status) = self.deep_validation.field_value() {
            summary.push_str(" deep=");
            summary.push_str(deep_status);
        }
        summary
    }

    pub fn event(&self) -> LogEvent<'static> {
        let mut event = LogEvent::info("runner", "config.validated", "runner config validated")
            .with_field("config", &self.config_path)
            .with_field("mappings", self.mappings)
            .with_field("webhook", self.webhook_bind_addr.to_string())
            .with_field("mode", self.webhook_mode);
        if let Some(tls) = &self.webhook_tls_files {
            event = event.with_field("tls", tls);
        }
        if let Some(deep_status) = self.deep_validation.field_value() {
            event = event.with_field("deep", deep_status);
        }
        event
    }
}

impl DeepValidationStatus {
    fn field_value(self) -> Option<&'static str> {
        match self {
            Self::Skipped => None,
            Self::Ok => Some("ok"),
        }
    }
}
