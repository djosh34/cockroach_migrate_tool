use std::{
    borrow::Cow,
    io::{self, Write},
    time::SystemTime,
};

use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Map, Value};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LogFormat {
    Text,
    Json,
}

impl LogFormat {
    pub fn writes_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[derive(Debug)]
pub struct LogEvent<'a> {
    service: &'a str,
    event: &'a str,
    level: &'a str,
    message: Cow<'a, str>,
    fields: Map<String, Value>,
}

impl<'a> LogEvent<'a> {
    pub fn info(service: &'a str, event: &'a str, message: impl Into<Cow<'a, str>>) -> Self {
        Self {
            service,
            event,
            level: "info",
            message: message.into(),
            fields: Map::new(),
        }
    }

    pub fn error(service: &'a str, event: &'a str, message: impl Into<Cow<'a, str>>) -> Self {
        Self {
            service,
            event,
            level: "error",
            message: message.into(),
            fields: Map::new(),
        }
    }

    pub fn with_field(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let value = serde_json::to_value(value).expect("log fields must serialize");
        self.fields.insert(key.into(), value);
        self
    }

    pub fn write_to(self, w: &mut impl Write, log_format: LogFormat) -> io::Result<()> {
        match log_format {
            LogFormat::Text => {
                writeln!(w, "{}", self.message)
            }
            LogFormat::Json => {
                let mut object = self.fields;
                object.insert("timestamp".into(), Value::String(timestamp_now()));
                object.insert("level".into(), Value::String(self.level.to_owned()));
                object.insert("service".into(), Value::String(self.service.to_owned()));
                object.insert("event".into(), Value::String(self.event.to_owned()));
                object.insert("message".into(), Value::String(self.message.into_owned()));
                serde_json::to_writer(&mut *w, &object)?;
                writeln!(w)
            }
        }
    }
}

fn timestamp_now() -> String {
    let system_time = SystemTime::now();
    let datetime = OffsetDateTime::from(system_time);
    datetime
        .format(&Rfc3339)
        .expect("rfc3339 formatting should succeed")
}
