//! A Solana RPC client that implements hedged requests across multiple providers.
//!
//! This library races RPC requests to multiple endpoints and returns the first successful
//! response, significantly reducing tail latency while maintaining reliability.
//!
//! # Quick Start
//!
//! ```no_run
//! use hedged_rpc_client::{HedgedRpcClient, HedgeConfig, ProviderConfig, ProviderId};
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let providers = vec![
//!     ProviderConfig {
//!         id: ProviderId("helius"),
//!         url: "https://mainnet.helius-rpc.com".to_string(),
//!     },
//!     ProviderConfig {
//!         id: ProviderId("triton"),
//!         url: "https://triton.helius.xyz".to_string(),
//!     },
//! ];
//!
//! let config = HedgeConfig::low_latency(providers.len());
//! let client = HedgedRpcClient::new(providers, config);
//!
//! let (provider, blockhash) = client.get_latest_blockhash().await?;
//! println!("Got blockhash from {}: {}", provider.0, blockhash);
//! # Ok(())
//! # }
//! ```
//!
//! # Hedging Strategy
//!
//! The client uses a configurable hedging strategy:
//! 1. Initially queries `initial_providers` endpoints
//! 2. If no response after `hedge_after` duration, fans out to more providers
//! 3. Returns the first successful response
//! 4. Times out after `overall_timeout` if all providers fail
//!
//! # Preset Configurations
//!
//! Use `HedgeConfig::low_latency()`, `::conservative()`, or `::aggressive()` for
//! common hedging strategies, or create a custom configuration.

pub mod client;
pub mod config;
pub mod errors;

pub use client::{HedgedRpcClient, ProviderStatsSnapshot};
pub use config::{HedgeConfig, ProviderConfig, ProviderId};
pub use errors::HedgedError;
pub use solana_sdk::{hash::Hash, pubkey::Pubkey};
