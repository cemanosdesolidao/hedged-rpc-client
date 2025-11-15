use std::time::Duration;

/// Unique identifier for an RPC provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderId(pub &'static str);

/// Configuration for a single RPC provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Unique identifier for this provider.
    pub id: ProviderId,
    /// RPC endpoint URL.
    pub url: String,
}

/// Hedging strategy configuration.
///
/// Controls how aggressively the client fans out requests to multiple providers.
/// The hedging strategy balances latency reduction with resource usage.
#[derive(Debug, Clone)]
pub struct HedgeConfig {
    /// Number of providers to query immediately when a request starts.
    ///
    /// Set to 1 for conservative hedging (only hedge if primary is slow),
    /// or higher to race multiple providers from the start.
    pub initial_providers: usize,

    /// Duration to wait before sending requests to additional providers.
    ///
    /// If no response is received within this time, the client will fan out
    /// to remaining providers (up to `max_providers`).
    pub hedge_after: Duration,

    /// Maximum number of providers to involve in a single request.
    ///
    /// Limits resource usage by capping concurrent requests per call.
    pub max_providers: usize,

    /// Optional minimum slot number for response validation.
    ///
    /// If set, responses with older slots are rejected as stale.
    pub min_slot: Option<u64>,

    /// Maximum time to wait for any provider to respond.
    ///
    /// If all providers fail to respond within this timeout, the request fails.
    pub overall_timeout: Duration,
}

impl Default for HedgeConfig {
    fn default() -> Self {
        Self {
            initial_providers: 1,
            hedge_after: Duration::from_millis(80),
            max_providers: usize::MAX,
            min_slot: None,
            overall_timeout: Duration::from_secs(2),
        }
    }
}

impl HedgeConfig {
    /// Creates a low-latency hedging configuration.
    ///
    /// Optimized for minimal response time with moderate resource usage:
    /// - Races 2 providers immediately
    /// - 20ms hedge delay
    /// - 1 second timeout
    pub fn low_latency(providers_len: usize) -> Self {
        Self {
            initial_providers: 2,
            hedge_after: Duration::from_millis(20),
            max_providers: providers_len,
            min_slot: None,
            overall_timeout: Duration::from_secs(1),
        }
    }

    /// Creates a conservative hedging configuration.
    ///
    /// Minimizes resource usage, only hedging if the primary provider is slow:
    /// - Queries 1 provider initially
    /// - 100ms hedge delay
    /// - 3 second timeout
    pub fn conservative(providers_len: usize) -> Self {
        Self {
            initial_providers: 1,
            hedge_after: Duration::from_millis(100),
            max_providers: providers_len,
            min_slot: None,
            overall_timeout: Duration::from_secs(3),
        }
    }

    /// Creates an aggressive hedging configuration.
    ///
    /// Prioritizes latency over resource usage:
    /// - Races 3 providers immediately
    /// - 20ms hedge delay
    /// - 1 second timeout
    pub fn aggressive(providers_len: usize) -> Self {
        Self {
            initial_providers: 3,
            hedge_after: Duration::from_millis(20),
            max_providers: providers_len,
            min_slot: None,
            overall_timeout: Duration::from_secs(1),
        }
    }
}
