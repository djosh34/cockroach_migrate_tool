use crate::config::PostgresConfig;

pub(crate) struct PostgresRuntime {
    endpoint_label: String,
}

impl PostgresRuntime {
    pub(crate) fn from_config(config: &PostgresConfig) -> Self {
        let _connect_options = sqlx::postgres::PgConnectOptions::new()
            .host(config.host())
            .port(config.port())
            .database(config.database())
            .username(config.user())
            .password(config.password());

        Self {
            endpoint_label: config.endpoint_label(),
        }
    }

    pub(crate) fn endpoint_label(&self) -> &str {
        &self.endpoint_label
    }
}
