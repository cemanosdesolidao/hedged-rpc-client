use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::Instant,
};

use futures::{stream::FuturesUnordered, StreamExt};
use solana_client::{client_error::ClientError, nonblocking::rpc_client::RpcClient};
use solana_commitment_config::CommitmentConfig;
use solana_rpc_client_api::{client_error::ErrorKind, response::Response as RpcResponse};
use solana_sdk::{account::Account, hash::Hash, pubkey::Pubkey};
use tokio::time;

use crate::{
    config::{HedgeConfig, ProviderConfig, ProviderId},
    errors::HedgedError,
};

#[derive(Debug, Default)]
struct ProviderStats {
    wins: u64,
    total_latency_ms: f64,
    errors: u64,
}

/// Snapshot of provider performance statistics.
#[derive(Debug, Clone)]
pub struct ProviderStatsSnapshot {
    /// Number of times this provider won the race.
    pub wins: u64,
    /// Average latency in milliseconds for winning calls.
    pub avg_latency_ms: f64,
    /// Number of failed calls from this provider.
    pub errors: u64,
}

/// A Solana RPC client that hedges requests across multiple providers.
///
/// The client races requests to multiple RPC endpoints and returns the first
/// successful response, implementing the "hedged requests" pattern to reduce
/// tail latency.
#[derive(Clone)]
pub struct HedgedRpcClient {
    providers: Arc<Vec<(ProviderId, Arc<RpcClient>)>>,
    cfg: HedgeConfig,
    stats: Arc<Mutex<HashMap<ProviderId, ProviderStats>>>,
}

impl HedgedRpcClient {
    /// Creates a new hedged RPC client with the specified providers and configuration.
    ///
    /// # Arguments
    /// * `provider_cfgs` - List of RPC provider configurations (URLs and IDs)
    /// * `cfg` - Hedging strategy configuration
    ///
    /// # Example
    /// ```no_run
    /// use hedged_rpc_client::{HedgedRpcClient, HedgeConfig, ProviderConfig, ProviderId};
    /// use std::time::Duration;
    ///
    /// let providers = vec![
    ///     ProviderConfig {
    ///         id: ProviderId("helius"),
    ///         url: "https://mainnet.helius-rpc.com".to_string(),
    ///     },
    /// ];
    ///
    /// let config = HedgeConfig {
    ///     initial_providers: 1,
    ///     hedge_after: Duration::from_millis(50),
    ///     max_providers: 3,
    ///     min_slot: None,
    ///     overall_timeout: Duration::from_secs(2),
    /// };
    ///
    /// let client = HedgedRpcClient::new(providers, config);
    /// ```
    pub fn new(provider_cfgs: Vec<ProviderConfig>, cfg: HedgeConfig) -> Self {
        let providers_vec: Vec<(ProviderId, Arc<RpcClient>)> = provider_cfgs
            .into_iter()
            .map(|pcfg| {
                let client = Arc::new(RpcClient::new(pcfg.url));
                (pcfg.id, client)
            })
            .collect();

        let mut stats_map = HashMap::new();
        for (id, _) in &providers_vec {
            stats_map.insert(*id, ProviderStats::default());
        }

        Self {
            providers: Arc::new(providers_vec),
            cfg,
            stats: Arc::new(Mutex::new(stats_map)),
        }
    }

    /// Returns a reference to the configured providers.
    pub fn providers(&self) -> &[(ProviderId, Arc<RpcClient>)] {
        &self.providers
    }

    /// Returns a snapshot of accumulated performance statistics for each provider.
    ///
    /// Statistics include wins (successful responses), average latency, and error counts.
    pub fn provider_stats(&self) -> HashMap<ProviderId, ProviderStatsSnapshot> {
        let stats = self.stats.lock().expect("provider stats mutex poisoned");

        stats
            .iter()
            .map(|(id, s)| {
                let avg = if s.wins > 0 {
                    s.total_latency_ms / (s.wins as f64)
                } else {
                    0.0
                };

                (
                    *id,
                    ProviderStatsSnapshot {
                        wins: s.wins,
                        avg_latency_ms: avg,
                        errors: s.errors,
                    },
                )
            })
            .collect()
    }

    /// Core hedged request implementation.
    ///
    /// Races the provided RPC call across multiple providers according to the configured
    /// hedging strategy. Returns the first successful response along with the provider ID.
    ///
    /// # Type Parameters
    /// * `T` - The response type
    /// * `F` - Closure that creates the RPC call
    /// * `Fut` - Future returned by the closure
    async fn hedged_call<T, F, Fut>(&self, f: F) -> Result<(ProviderId, T), HedgedError>
    where
        T: Send,
        F: Fn(Arc<RpcClient>) -> Fut + Send,
        Fut: Future<Output = Result<T, ClientError>> + Send,
    {
        if self.providers.is_empty() {
            return Err(HedgedError::NoProviders);
        }

        let max_idx = self.cfg.max_providers.min(self.providers.len());
        if max_idx == 0 {
            return Err(HedgedError::NoProviders);
        }
        let selected_providers = &self.providers[..max_idx];

        let start = Instant::now();
        let selected_ids: Vec<ProviderId> = selected_providers.iter().map(|(id, _)| *id).collect();

        let hedging_logic = async {
            let mut failures = Vec::new();
            let mut futures = FuturesUnordered::new();

            let spawn_provider = move |provider_id: ProviderId, client: Arc<RpcClient>| {
                let fut = f(client);
                async move {
                    let result = fut.await;
                    (provider_id, result)
                }
            };

            let initial_count = self
                .cfg
                .initial_providers
                .max(1)
                .min(selected_providers.len());

            for (provider_id, client) in &selected_providers[..initial_count] {
                futures.push(spawn_provider(*provider_id, client.clone()));
            }

            let needs_hedging = initial_count < selected_providers.len();
            let mut hedged = !needs_hedging;
            let hedge_sleep = time::sleep(self.cfg.hedge_after);
            tokio::pin!(hedge_sleep);

            loop {
                if futures.is_empty() && hedged {
                    break;
                }

                tokio::select! {
                    Some((provider_id, result)) = futures.next(), if !futures.is_empty() => {
                        match result {
                            Ok(val) => return Ok((provider_id, val)),
                            Err(e) => failures.push((provider_id, e)),
                        }
                    }
                    _ = &mut hedge_sleep, if needs_hedging && !hedged => {
                        hedged = true;
                        for (provider_id, client) in &selected_providers[initial_count..] {
                            futures.push(spawn_provider(*provider_id, client.clone()));
                        }
                    }
                }
            }

            Err(HedgedError::AllFailed(failures))
        };

        let timed = time::timeout(self.cfg.overall_timeout, hedging_logic).await;

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        match timed {
            Err(_) => {
                if let Ok(mut stats) = self.stats.lock() {
                    for id in selected_ids {
                        if let Some(entry) = stats.get_mut(&id) {
                            entry.errors += 1;
                        }
                    }
                }
                Err(HedgedError::Timeout(self.cfg.overall_timeout))
            }
            Ok(inner) => match inner {
                Ok((winner_id, value)) => {
                    if let Ok(mut stats) = self.stats.lock() {
                        if let Some(entry) = stats.get_mut(&winner_id) {
                            entry.wins += 1;
                            entry.total_latency_ms += elapsed_ms;
                        }
                    }
                    Ok((winner_id, value))
                }
                Err(HedgedError::AllFailed(failures)) => {
                    if let Ok(mut stats) = self.stats.lock() {
                        for (id, _err) in failures.iter() {
                            if let Some(entry) = stats.get_mut(id) {
                                entry.errors += 1;
                            }
                        }
                    }
                    Err(HedgedError::AllFailed(failures))
                }
                Err(e) => Err(e),
            },
        }
    }

    /// Gets the latest blockhash from the fastest responding provider.
    ///
    /// Returns the blockhash along with the ID of the provider that responded first.
    pub async fn get_latest_blockhash(&self) -> Result<(ProviderId, Hash), HedgedError> {
        let (id, resp) = self
            .hedged_call(move |client| async move { client.get_latest_blockhash().await })
            .await?;

        Ok((id, resp))
    }

    /// Gets the latest blockhash, returning only the hash without provider information.
    pub async fn get_latest_blockhash_any(&self) -> Result<Hash, HedgedError> {
        let (_id, resp) = self.get_latest_blockhash().await?;
        Ok(resp)
    }

    /// Gets account data from the fastest responding provider.
    ///
    /// Returns the account response along with the ID of the provider that responded first.
    ///
    /// # Arguments
    /// * `pubkey` - The account's public key
    /// * `commitment` - The commitment level for the query
    pub async fn get_account(
        &self,
        pubkey: &Pubkey,
        commitment: CommitmentConfig,
    ) -> Result<(ProviderId, RpcResponse<Option<Account>>), HedgedError> {
        let pk = *pubkey;

        let (id, resp) = self
            .hedged_call(move |client| {
                let pk = pk;
                async move { client.get_account_with_commitment(&pk, commitment).await }
            })
            .await?;

        Ok((id, resp))
    }

    /// Gets account data, returning only the response without provider information.
    pub async fn get_account_any(
        &self,
        pubkey: &Pubkey,
        commitment: CommitmentConfig,
    ) -> Result<RpcResponse<Option<Account>>, HedgedError> {
        let (_id, resp) = self.get_account(pubkey, commitment).await?;

        Ok(resp)
    }

    /// Gets account data with slot freshness validation.
    ///
    /// Returns an error if the response slot is older than the specified minimum slot.
    /// Useful for ensuring data recency in time-sensitive operations.
    ///
    /// # Arguments
    /// * `pubkey` - The account's public key
    /// * `commitment` - The commitment level for the query
    /// * `min_slot` - Minimum acceptable slot number
    pub async fn get_account_fresh(
        &self,
        pubkey: &Pubkey,
        commitment: CommitmentConfig,
        min_slot: u64,
    ) -> Result<(ProviderId, RpcResponse<Option<Account>>), HedgedError> {
        let (id, resp) = self.get_account(pubkey, commitment).await?;
        if resp.context.slot < min_slot {
            return Err(HedgedError::AllFailed(vec![(
                id,
                ErrorKind::Custom(format!(
                    "StaleResponse: min_slot {min_slot}, got {}",
                    resp.context.slot
                ))
                .into(),
            )]));
        }
        Ok((id, resp))
    }
}
