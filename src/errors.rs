use std::time::Duration;

use solana_client::client_error::ClientError;

use crate::config::ProviderId;

/// Errors that can occur during hedged RPC operations.
#[derive(thiserror::Error, Debug)]
pub enum HedgedError {
    /// No RPC providers were configured.
    #[error("no providers configured")]
    NoProviders,

    /// All configured providers returned errors.
    ///
    /// Contains the list of providers and their individual errors.
    #[error("all providers failed: {0:?}")]
    AllFailed(Vec<(ProviderId, ClientError)>),

    /// The hedged request exceeded the configured timeout.
    ///
    /// None of the providers responded successfully within the time limit.
    #[error("hedged call timed out after {0:?}")]
    Timeout(Duration),
}
