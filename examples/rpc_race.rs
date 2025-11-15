//! High-volume stress test for hedged RPC requests.
//!
//! This example performs 50,000 concurrent RPC calls with rate limiting,
//! demonstrating the hedged client's performance under heavy load.
//! Useful for benchmarking and comparing provider performance.

use std::{
    collections::HashMap,
    env,
    sync::Arc,
    time::{Duration, Instant},
};

use hedged_rpc_client::{HedgeConfig, HedgedRpcClient, ProviderConfig, ProviderId};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::{mpsc, Semaphore};

const NUM_CALLS: usize = 50_000;
const MAX_IN_FLIGHT: usize = 256;

#[derive(Debug)]
enum CallOutcome {
    Ok {
        provider: ProviderId,
        latency: Duration,
    },
    Err {
        error: String,
        latency: Duration,
    },
}

#[derive(Debug)]
struct CallResult {
    call_idx: usize,
    outcome: CallOutcome,
}

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

    let cfg = HedgeConfig {
        initial_providers: providers.len(),
        hedge_after: Duration::from_millis(20),
        max_providers: providers.len(),
        min_slot: None,
        overall_timeout: Duration::from_secs(1),
    };

    let client = HedgedRpcClient::new(providers, cfg);

    let addr: Pubkey = "So11111111111111111111111111111111111111112".parse()?;
    let commitment = CommitmentConfig::processed();

    let (tx, mut rx) = mpsc::channel::<CallResult>(MAX_IN_FLIGHT * 2);
    let semaphore = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
    let consumer = tokio::spawn(async move {
        let mut results: Vec<CallResult> = Vec::with_capacity(NUM_CALLS);

        while let Some(res) = rx.recv().await {
            match &res.outcome {
                CallOutcome::Ok { provider, latency } => {
                    println!(
                        "[call {:05}] OK   provider={} latency={:?}",
                        res.call_idx, provider.0, latency
                    );
                }
                CallOutcome::Err { error, latency } => {
                    println!(
                        "[call {:05}] ERR  latency={:?} error={}",
                        res.call_idx, latency, error
                    );
                }
            }

            results.push(res);
        }

        results
    });

    for i in 0..NUM_CALLS {
        let client_clone = client.clone();
        let tx_clone = tx.clone();
        let addr_copy = addr;
        let commitment_clone = commitment;
        let sem = semaphore.clone();

        tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore closed");

            let start = Instant::now();
            let res = client_clone.get_account(&addr_copy, commitment_clone).await;
            let elapsed = start.elapsed();

            let outcome = match res {
                Ok((provider, _resp)) => CallOutcome::Ok {
                    provider,
                    latency: elapsed,
                },
                Err(e) => CallOutcome::Err {
                    error: e.to_string(),
                    latency: elapsed,
                },
            };

            let _ = tx_clone
                .send(CallResult {
                    call_idx: i,
                    outcome,
                })
                .await;
        });
    }

    drop(tx);
    let mut results = consumer.await?;

    results.sort_by_key(|r| r.call_idx);

    let mut wins: HashMap<&'static str, usize> = HashMap::new();
    let mut total_latency: HashMap<&'static str, Duration> = HashMap::new();
    let mut error_count = 0usize;

    for r in &results {
        match &r.outcome {
            CallOutcome::Ok { provider, latency } => {
                let name = provider.0;
                *wins.entry(name).or_insert(0) += 1;
                *total_latency.entry(name).or_insert(Duration::ZERO) += *latency;
            }
            CallOutcome::Err { .. } => {
                error_count += 1;
            }
        }
    }

    println!("\n=== summary ===");
    println!("total calls          : {}", NUM_CALLS);
    println!("successes            : {}", NUM_CALLS - error_count);
    println!("errors (any kind)    : {}", error_count);

    for (provider, count) in wins.iter() {
        let total = total_latency[provider];
        let avg_ms = total.as_secs_f64() * 1000.0 / (*count as f64);
        println!(
            "provider {:>10}: wins = {:6}, avg_latency = {:8.3} ms",
            provider, count, avg_ms,
        );
    }

    Ok(())
}
