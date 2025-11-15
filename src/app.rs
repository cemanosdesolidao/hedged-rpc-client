//! Application state and logic for the TUI dashboard.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use color_eyre::Result;
use hedged_rpc_client::{
    config::{ProviderConfig, ProviderId},
    HedgedRpcClient, ProviderStatsSnapshot,
};
use solana_sdk::pubkey::Pubkey;

/// Operating mode for RPC calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Race multiple providers (hedged requests).
    Hedged,
    /// Query only the selected provider.
    SingleProvider,
}

/// RPC method to call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Fetch the latest blockhash.
    LatestBlockhash,
    /// Fetch account data for the configured target account.
    GetAccount,
}

/// Events emitted by RPC operations.
#[derive(Debug)]
pub enum AppEvent {
    /// An RPC call completed.
    RpcFinished {
        provider: Option<ProviderId>,
        latency_ms: f64,
        ok: bool,
        message: String,
    },
}

/// Main application state for the TUI.
pub struct App {
    pub client: HedgedRpcClient,
    pub providers: Vec<(ProviderId, String)>,
    pub selected_idx: usize,
    pub mode: Mode,
    pub method: Method,
    pub last_message: String,
    pub last_provider: Option<ProviderId>,
    pub last_latency_ms: Option<f64>,
    pub stats_snapshot: HashMap<ProviderId, ProviderStatsSnapshot>,
    pub target_account: Pubkey,
    pub batch_mode: bool,
    pub batch_count: usize,
    pub batch_current: usize,
    pub provider_count: usize,
    pub session_start: Instant,
    pub total_calls: u64,
    pub total_successes: u64,
    pub total_errors: u64,
    pub latency_history: HashMap<ProviderId, VecDeque<u64>>,
    pub call_timestamps: VecDeque<Instant>,
}

impl App {
    pub fn new(client: HedgedRpcClient, providers_cfg: Vec<ProviderConfig>) -> Result<Self> {
        let providers: Vec<(ProviderId, String)> = providers_cfg
            .into_iter()
            .map(|pcfg| (pcfg.id, pcfg.url))
            .collect();

        let target_account: Pubkey = "So11111111111111111111111111111111111111112".parse()?;
        let provider_count = providers.len();

        let mut latency_history = HashMap::new();
        for (id, _) in &providers {
            latency_history.insert(*id, VecDeque::with_capacity(100));
        }

        Ok(Self {
            client,
            providers,
            selected_idx: 0,
            mode: Mode::Hedged,
            method: Method::GetAccount,
            last_message: String::from("Ready. Press 'r' to run a call or 'b' for batch mode"),
            last_provider: None,
            last_latency_ms: None,
            stats_snapshot: HashMap::new(),
            target_account,
            batch_mode: false,
            batch_count: 10,
            batch_current: 0,
            provider_count,
            session_start: Instant::now(),
            total_calls: 0,
            total_successes: 0,
            total_errors: 0,
            latency_history,
            call_timestamps: VecDeque::with_capacity(1000),
        })
    }

    pub fn next_provider(&mut self) {
        if !self.providers.is_empty() {
            self.selected_idx = (self.selected_idx + 1) % self.providers.len();
        }
    }

    pub fn prev_provider(&mut self) {
        if !self.providers.is_empty() {
            if self.selected_idx == 0 {
                self.selected_idx = self.providers.len() - 1;
            } else {
                self.selected_idx -= 1;
            }
        }
    }

    pub fn increase_provider_count(&mut self) {
        if self.provider_count < self.providers.len() {
            self.provider_count += 1;
        }
    }

    pub fn decrease_provider_count(&mut self) {
        if self.provider_count > 1 {
            self.provider_count -= 1;
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            Mode::Hedged => Mode::SingleProvider,
            Mode::SingleProvider => Mode::Hedged,
        };
    }

    pub fn toggle_method(&mut self) {
        self.method = match self.method {
            Method::LatestBlockhash => Method::GetAccount,
            Method::GetAccount => Method::LatestBlockhash,
        };
    }

    pub fn toggle_batch_mode(&mut self) {
        self.batch_mode = !self.batch_mode;
        if self.batch_mode {
            self.batch_current = 0;
            self.last_message = format!("Batch mode ON: {} calls queued", self.batch_count);
        } else {
            self.last_message = "Batch mode OFF".to_string();
        }
    }

    pub fn increase_batch_count(&mut self) {
        self.batch_count = (self.batch_count + 10).min(1000);
    }

    pub fn decrease_batch_count(&mut self) {
        self.batch_count = (self.batch_count.saturating_sub(10)).max(10);
    }

    pub fn refresh_stats(&mut self) {
        let client_stats = self.client.provider_stats();
        for (id, stats) in client_stats {
            self.stats_snapshot.insert(id, stats);
        }
    }

    pub fn update_stats_for_call(
        &mut self,
        provider: Option<ProviderId>,
        latency_ms: f64,
        ok: bool,
    ) {
        if let Some(provider_id) = provider {
            let entry =
                self.stats_snapshot
                    .entry(provider_id)
                    .or_insert_with(|| ProviderStatsSnapshot {
                        wins: 0,
                        avg_latency_ms: 0.0,
                        errors: 0,
                    });

            if ok {
                let total_latency = entry.avg_latency_ms * (entry.wins as f64);
                entry.wins += 1;
                entry.avg_latency_ms = (total_latency + latency_ms) / (entry.wins as f64);
            } else {
                entry.errors += 1;
            }
        }
    }

    pub fn set_last_result(
        &mut self,
        provider: Option<ProviderId>,
        latency_ms: f64,
        ok: bool,
        message: String,
    ) {
        self.last_provider = provider;
        self.last_latency_ms = Some(latency_ms);

        self.update_stats_for_call(provider, latency_ms, ok);

        self.total_calls += 1;
        if ok {
            self.total_successes += 1;
        } else {
            self.total_errors += 1;
        }

        if let Some(provider_id) = provider {
            let history = self
                .latency_history
                .entry(provider_id)
                .or_insert_with(|| VecDeque::with_capacity(100));
            history.push_back(latency_ms as u64);
            if history.len() > 100 {
                history.pop_front();
            }
        }

        self.call_timestamps.push_back(Instant::now());
        if self.call_timestamps.len() > 1000 {
            self.call_timestamps.pop_front();
        }

        let status = if ok { "✓" } else { "✗" };
        self.last_message = format!("{} {} ({:.0} ms)", status, message, latency_ms);

        if self.batch_mode {
            self.batch_current += 1;
            if self.batch_current >= self.batch_count {
                self.batch_mode = false;
                self.last_message = format!("Batch complete! {} calls finished", self.batch_count);
            }
        }
    }

    pub fn selected_provider_id(&self) -> Option<ProviderId> {
        self.providers.get(self.selected_idx).map(|(id, _)| *id)
    }

    pub fn mode_string(&self) -> String {
        match self.mode {
            Mode::Hedged => format!("Hedged ({} providers)", self.provider_count),
            Mode::SingleProvider => "Single Provider".to_string(),
        }
    }

    pub fn should_run_call(&self) -> bool {
        self.batch_mode && self.batch_current < self.batch_count
    }

    pub fn session_uptime(&self) -> Duration {
        self.session_start.elapsed()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_calls > 0 {
            (self.total_successes as f64 / self.total_calls as f64) * 100.0
        } else {
            0.0
        }
    }

    pub fn calls_per_second(&self) -> f64 {
        let now = Instant::now();
        let one_sec_ago = now - Duration::from_secs(1);

        self.call_timestamps
            .iter()
            .filter(|&&ts| ts > one_sec_ago)
            .count() as f64
    }

    pub fn average_latency(&self) -> f64 {
        let mut total = 0u64;
        let mut count = 0usize;

        for history in self.latency_history.values() {
            for &latency in history {
                total += latency;
                count += 1;
            }
        }

        if count > 0 {
            total as f64 / count as f64
        } else {
            0.0
        }
    }
}
