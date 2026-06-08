//! Logging subsystem for Alius Core Runtime.
//!
//! Provides structured JSONL-based logging with real-time subscription,
//! sensitive data redaction, and append-only event log for audit.

pub mod audit;
pub mod error;
pub mod event_log;
pub mod log_query;
pub mod log_record;
pub mod log_writer;

pub use error::LoggingError;
pub use event_log::EventLogEntry;
pub use log_query::LogQuery;
pub use log_record::{LogLevel, LogRecord};
pub use log_writer::LogWriter;
