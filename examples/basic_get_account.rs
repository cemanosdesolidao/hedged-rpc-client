//! Basic example demonstrating hedged RPC requests.
//!
//! This example shows how to use the hedged RPC client to fetch a blockhash
//! and account data, with the client automatically racing multiple providers.

use std::{
    env,
    time::{Duration, Instant},
};

use hedged_rpc_client::{HedgeConfig, HedgedRpcClient, ProviderConfig, ProviderId};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

fn provider_from_env(env_key: &str, id: &'static str) -> Option<ProviderConfig> {
    env::var(env_key).ok().map(|url| ProviderConfig {
        id: ProviderId(id),
        url,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        eprintln!("No providers configured.");
        eprintln!("Set at least one of: HELIUS_RPC_URL, TRITON_RPC_URL, QUICKNODE_RPC_URL");
        return Ok(());
    }

    eprintln!(
        "Using providers:\n{}",
        providers
            .iter()
            .map(|p| format!("- {}: {}", (p.id).0, p.url))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let cfg = HedgeConfig {
        initial_providers: 1,
        hedge_after: Duration::from_millis(80),
        max_providers: providers.len(),
        min_slot: None,
        overall_timeout: Duration::from_secs(2),
    };

    let client = HedgedRpcClient::new(providers, cfg);

    let addr: Pubkey = "So11111111111111111111111111111111111111112".parse()?;
    let commitment = CommitmentConfig::processed();

    let t0 = Instant::now();
    let (bh_provider, blockhash) = client.get_latest_blockhash().await?;
    let dt_bh = t0.elapsed();

    println!(
        "[blockhash] provider={} latency={:?} hash={}",
        bh_provider.0, dt_bh, blockhash,
    );

    let t1 = Instant::now();
    let (acc_provider, resp) = client.get_account(&addr, commitment).await?;
    let dt_acc = t1.elapsed();

    println!(
        "[account]   provider={} latency={:?} slot={}",
        acc_provider.0, dt_acc, resp.context.slot,
    );

    match resp.value {
        Some(account) => {
            println!("lamports={}", account.lamports);
            println!("data_len={}", account.data.len());
            println!("owner={}", account.owner);
        }
        None => println!("account not found"),
    }

    Ok(())
}
