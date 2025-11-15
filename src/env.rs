//! Environment configuration utilities for loading RPC providers from environment variables.

use std::{env, time::Duration};

use color_eyre::Result;
use hedged_rpc_client::{
    config::{HedgeConfig, ProviderConfig, ProviderId},
    HedgedRpcClient,
};

/// Attempts to load a provider configuration from an environment variable.
///
/// Returns `None` if the environment variable is not set.
pub fn provider_from_env(env_key: &str, id: &'static str) -> Option<ProviderConfig> {
    env::var(env_key).ok().map(|url| ProviderConfig {
        id: ProviderId(id),
        url,
    })
}

/// Builds a hedged RPC client from environment variables.
///
/// Looks for the following environment variables:
/// - `HELIUS_RPC_URL`
/// - `TRITON_RPC_URL`
/// - `QUICKNODE_RPC_URL`
///
/// Returns an error if no providers are configured.
pub fn build_client_from_env() -> Result<(HedgedRpcClient, Vec<ProviderConfig>)> {
    let mut providers = Vec::new();

    if let Some(p) = provider_from_env("HELIUS_RPC_URL", "helius") {
        providers.push(p);
    }
    if let Some(p) = provider_from_env("TRITON_RPC_URL", "triton") {
        providers.push(p);
    }
    if let Some(p) = provider_from_env("QUICKNODE_RPC_URL", "quicknode") {
        providers.push(p);
    }

    if providers.is_empty() {
        color_eyre::eyre::bail!(
            "No providers configured.\n\
             Set at least one of: HELIUS_RPC_URL, TRITON_RPC_URL, QUICKNODE_RPC_URL"
        );
    }

    let cfg = HedgeConfig {
        initial_providers: 1,
        hedge_after: Duration::from_millis(50),
        max_providers: providers.len(),
        min_slot: None,
        overall_timeout: Duration::from_secs(2),
    };

    let client = HedgedRpcClient::new(providers.clone(), cfg);
    Ok((client, providers))
}
