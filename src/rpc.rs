//! RPC call execution logic for the TUI dashboard.

use std::time::{Duration, Instant};

use color_eyre::Result;
use hedged_rpc_client::{
    config::{HedgeConfig, ProviderConfig, ProviderId},
    HedgedRpcClient,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use tokio::sync::mpsc;

use crate::app::{App, AppEvent, Method, Mode};

/// Spawns an asynchronous RPC call based on the current app configuration.
///
/// The call is executed in a background task and sends the result via the provided channel.
pub fn spawn_rpc_call(app: &App, tx: mpsc::UnboundedSender<AppEvent>) {
    let mode = app.mode;
    let method = app.method;
    let selected_idx = app.selected_idx;
    let providers = app.providers.clone();
    let target_pubkey = app.target_account;
    let commitment = CommitmentConfig::processed();
    let provider_count = app.provider_count;

    tokio::spawn(async move {
        let start = Instant::now();

        let result: (Option<ProviderId>, Result<String>) = match (mode, method) {
            (Mode::Hedged, Method::LatestBlockhash) => {
                let hedged_client = create_hedged_client(&providers, provider_count);
                let res = hedged_client.get_latest_blockhash().await;
                match res {
                    Ok((id, hash)) => (Some(id), Ok(format!("hash={hash}"))),
                    Err(e) => (None, Err(e.into())),
                }
            }
            (Mode::Hedged, Method::GetAccount) => {
                let hedged_client = create_hedged_client(&providers, provider_count);
                let res = hedged_client.get_account(&target_pubkey, commitment).await;
                match res {
                    Ok((id, resp)) => {
                        let slot = resp.context.slot;
                        let lamports = resp.value.as_ref().map(|acc| acc.lamports).unwrap_or(0);
                        (Some(id), Ok(format!("slot={slot}, lamports={lamports}")))
                    }
                    Err(e) => (None, Err(e.into())),
                }
            }
            (Mode::SingleProvider, Method::LatestBlockhash) => {
                if let Some((id, rpc_url)) = providers.get(selected_idx) {
                    let id = *id;
                    let rpc_client = RpcClient::new(rpc_url.clone());
                    match rpc_client.get_latest_blockhash().await {
                        Ok(hash) => (Some(id), Ok(format!("hash={hash}"))),
                        Err(e) => (Some(id), Err(e.into())),
                    }
                } else {
                    (None, Err(color_eyre::eyre::eyre!("No provider selected")))
                }
            }
            (Mode::SingleProvider, Method::GetAccount) => {
                if let Some((id, rpc_url)) = providers.get(selected_idx) {
                    let id = *id;
                    let rpc_client = RpcClient::new(rpc_url.clone());
                    match rpc_client
                        .get_account_with_commitment(&target_pubkey, commitment)
                        .await
                    {
                        Ok(resp) => {
                            let slot = resp.context.slot;
                            let lamports = resp.value.as_ref().map(|acc| acc.lamports).unwrap_or(0);
                            (Some(id), Ok(format!("slot={slot}, lamports={lamports}")))
                        }
                        Err(e) => (Some(id), Err(e.into())),
                    }
                } else {
                    (None, Err(color_eyre::eyre::eyre!("No provider selected")))
                }
            }
        };

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (provider, ok, msg) = match result {
            (prov, Ok(msg)) => (prov, true, msg),
            (prov, Err(e)) => (prov, false, e.to_string()),
        };

        let _ = tx.send(AppEvent::RpcFinished {
            provider,
            latency_ms: elapsed_ms,
            ok,
            message: msg,
        });
    });
}

fn create_hedged_client(
    providers: &[(ProviderId, String)],
    provider_count: usize,
) -> HedgedRpcClient {
    let limited_providers: Vec<_> = providers
        .iter()
        .take(provider_count)
        .map(|(id, url)| ProviderConfig {
            id: *id,
            url: url.clone(),
        })
        .collect();

    let cfg = HedgeConfig {
        initial_providers: 1,
        hedge_after: Duration::from_millis(50),
        max_providers: provider_count,
        min_slot: None,
        overall_timeout: Duration::from_secs(2),
    };

    HedgedRpcClient::new(limited_providers, cfg)
}
