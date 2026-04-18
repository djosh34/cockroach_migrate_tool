use std::time::Duration;

use crate::config::ReconcileConfig;

pub(crate) struct ReconcileRuntime {
    interval: Duration,
}

impl ReconcileRuntime {
    pub(crate) fn from_config(config: &ReconcileConfig) -> Self {
        Self {
            interval: Duration::from_secs(config.interval_secs()),
        }
    }

    pub(crate) fn interval(&self) -> Duration {
        self.interval
    }
}
