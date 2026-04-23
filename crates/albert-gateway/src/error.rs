//! Gateway error type.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("mock gateway is already running")]
    AlreadyRunning,
    #[error("mock gateway is not running")]
    NotRunning,
    #[error("failed to bind to {addr}: {source}")]
    Bind {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("gateway task panicked: {0}")]
    JoinPanic(String),
    #[error("invalid gateway configuration: {0}")]
    InvalidConfig(String),
}
