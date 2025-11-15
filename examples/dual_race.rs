//! Dual runner comparison test for hedged RPC performance.
//!
//! This example spawns two independent test runners that perform 10,000 calls each,
//! allowing direct comparison of hedged client behavior and provider performance
//! across concurrent workloads.

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

#[derive(Debug)]
struct RunnerStats {
    label: &'static str,
    total_calls: usize,
    successes: usize,
    errors: usize,
    avg_latency_ms: f64,
    per_provider_wins: HashMap<&'static str, usize>,
}

fn provider_from_env(env_key: &str, id: &'static str) -> Option<ProviderConfig> {
    env::var(env_key).ok().map(|url| ProviderConfig {
        id: ProviderId(id),
        url,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
        initial_providers: 2,
        hedge_after: Duration::from_millis(20),
        max_providers: providers.len(),
        min_slot: None,
        overall_timeout: Duration::from_secs(2),
    };

    let client_a = HedgedRpcClient::new(providers.clone(), cfg.clone());
    let client_b = HedgedRpcClient::new(providers, cfg);

    let addr: Pubkey = "So11111111111111111111111111111111111111112".parse()?;
    let commitment = CommitmentConfig::processed();
    let num_calls_per_runner: usize = 10_000;
    let max_in_flight_per_runner: usize = 256;

    let runner_a = tokio::spawn(run_runner(
        "A",
        client_a,
        addr,
        commitment,
        num_calls_per_runner,
        max_in_flight_per_runner,
    ));

    let runner_b = tokio::spawn(run_runner(
        "B",
        client_b,
        addr,
        commitment,
        num_calls_per_runner,
        max_in_flight_per_runner,
    ));

    let stats_a = runner_a.await??;
    let stats_b = runner_b.await??;

    println!("\n=== comparison ===");
    println!(
        "Runner {}: total={}, successes={}, errors={}, avg_latency={:.3} ms",
        stats_a.label,
        stats_a.total_calls,
        stats_a.successes,
        stats_a.errors,
        stats_a.avg_latency_ms
    );
    for (provider, wins) in &stats_a.per_provider_wins {
        println!(
            "  [{}] wins from provider {} = {}",
            stats_a.label, provider, wins
        );
    }

    println!(
        "Runner {}: total={}, successes={}, errors={}, avg_latency={:.3} ms",
        stats_b.label,
        stats_b.total_calls,
        stats_b.successes,
        stats_b.errors,
        stats_b.avg_latency_ms
    );
    for (provider, wins) in &stats_b.per_provider_wins {
        println!(
            "  [{}] wins from provider {} = {}",
            stats_b.label, provider, wins
        );
    }

    if stats_a.avg_latency_ms < stats_b.avg_latency_ms {
        println!(
            "\n=> Runner {} was faster on average by {:.3} ms",
            stats_a.label,
            stats_b.avg_latency_ms - stats_a.avg_latency_ms
        );
    } else if stats_b.avg_latency_ms < stats_a.avg_latency_ms {
        println!(
            "\n=> Runner {} was faster on average by {:.3} ms",
            stats_b.label,
            stats_a.avg_latency_ms - stats_b.avg_latency_ms
        );
    } else {
        println!("\n=> Both runners had the same average latency.");
    }

    Ok(())
}

async fn run_runner(
    label: &'static str,
    client: HedgedRpcClient,
    addr: Pubkey,
    commitment: CommitmentConfig,
    num_calls: usize,
    max_in_flight: usize,
) -> Result<RunnerStats, Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel::<CallResult>(max_in_flight * 2);
    let semaphore = Arc::new(Semaphore::new(max_in_flight));

    let consumer = tokio::spawn(async move {
        let mut results: Vec<CallResult> = Vec::with_capacity(num_calls);

        while let Some(res) = rx.recv().await {
            match &res.outcome {
                CallOutcome::Ok { provider, latency } => {
                    println!(
                        "[{} call {:05}] OK   provider={} latency={:?}",
                        label, res.call_idx, provider.0, latency
                    );
                }
                CallOutcome::Err { error, latency } => {
                    println!(
                        "[{} call {:05}] ERR  latency={:?} error={}",
                        label, res.call_idx, latency, error
                    );
                }
            }

            results.push(res);
        }

        results
    });

    for i in 0..num_calls {
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
    let results = consumer.await?;

    let mut successes = 0usize;
    let mut errors = 0usize;
    let mut sum_latency = Duration::ZERO;
    let mut per_provider_wins: HashMap<&'static str, usize> = HashMap::new();

    for r in &results {
        match &r.outcome {
            CallOutcome::Ok { provider, latency } => {
                successes += 1;
                sum_latency += *latency;
                *per_provider_wins.entry(provider.0).or_insert(0) += 1;
            }
            CallOutcome::Err { .. } => {
                errors += 1;
            }
        }
    }

    let avg_latency_ms = if successes > 0 {
        (sum_latency.as_secs_f64() * 1000.0) / (successes as f64)
    } else {
        0.0
    };

    Ok(RunnerStats {
        label,
        total_calls: num_calls,
        successes,
        errors,
        avg_latency_ms,
        per_provider_wins,
    })
}
