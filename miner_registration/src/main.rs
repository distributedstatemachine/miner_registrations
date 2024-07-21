//! This module implements a registration script for a blockchain network.
//! It allows users to register hotkeys using provided coldkeys and other parameters.

use clap::Parser;
use log::{error, info, warn};
use scale_value::{Composite, Value};
use serde::Deserialize;
use shared::{estimate_block_time, parse_config};
use sp_core::H256;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use subxt::ext::sp_core::{sr25519, Pair};
use subxt::tx::DefaultPayload;
use subxt::{tx::PairSigner, OnlineClient, SubstrateConfig};
use tokio::sync::Mutex;

/// Struct to hold registration parameters, can be parsed from command line or config file
#[derive(Parser, Deserialize, Debug)]
#[clap(author, version, about, long_about = None)]
struct RegistrationParams {
    #[clap(long)]
    coldkey: String,

    #[clap(long)]
    hotkey: String,

    #[clap(long)]
    netuid: u16,

    #[clap(long)]
    max_cost: u64,

    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    chain_endpoint: String,
}

/// Returns the current date and time in Eastern Time Zone
///
/// # Returns
///
/// A `String` representing the current date and time in the format "YYYY-MM-DD HH:MM:SS TimeZone"
fn get_formatted_date_now() -> String {
    let now = chrono::Utc::now();
    let eastern_time = now.with_timezone(&chrono_tz::US::Eastern);
    eastern_time.format("%Y-%m-%d %H:%M:%S %Z%z").to_string()
}

/// Attempts to register a hotkey on the blockchain
///
/// # Arguments
///
/// * `params` - A reference to `RegistrationParams` containing registration details
///
/// # Returns
///
/// A `Result` which is `Ok` if registration is successful, or an `Err` containing the error message
// TODO: Parse event and decode Registered event
async fn register_hotkey(params: &RegistrationParams) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize client connection to the blockchain
    let client = Arc::new(OnlineClient::<SubstrateConfig>::from_url(&params.chain_endpoint).await?);

    // Parse coldkey and hotkey from provided strings
    let coldkey: sr25519::Pair =
        sr25519::Pair::from_string(&params.coldkey, None).map_err(|_| "Invalid coldkey")?;
    let hotkey: sr25519::Pair =
        sr25519::Pair::from_string(&params.hotkey, None).map_err(|_| "Invalid hotkey")?;

    let signer = Arc::new(PairSigner::new(coldkey.clone()));

    let mut blocks = client.blocks().subscribe_finalized().await?;
    let last_attempt = Arc::new(Mutex::new(Instant::now()));
    let loops = Arc::new(Mutex::new(0u64));

    // Cache the call_data for efficiency
    let call_data = Arc::new(Composite::named([
        ("netuid", params.netuid.into()),
        ("hotkey", hotkey.public().0.to_vec().into()),
    ]));

    // Main registration loop
    while let Some(block) = blocks.next().await {
        let block = block?;
        let block_number = block.header().number;

        // Increment and log loop count
        {
            let mut loops_guard = loops.lock().await;
            *loops_guard += 1;
            info!(
                "{} | {} | Attempting registration for block {}",
                *loops_guard,
                get_formatted_date_now(),
                block_number
            );
        }

        // Check recycle cost
        let recycle_cost_start = Instant::now();
        let recycle_cost = get_recycle_cost(&client, params.netuid).await?;
        let recycle_cost_duration = recycle_cost_start.elapsed();
        info!("⏱️ get_recycle_cost took {:?}", recycle_cost_duration);

        // Skip if cost exceeds maximum allowed
        if recycle_cost > params.max_cost {
            warn!(
                "💸 Recycle cost ({}) exceeds threshold ({}). Skipping registration attempt.",
                recycle_cost, params.max_cost
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        // Prepare transaction payload
        let payload = DefaultPayload::new(
            "SubtensorModule",
            "burned_register",
            call_data.as_ref().clone(),
        );

        // Sign and submit the transaction
        let sign_and_submit_start: Instant = Instant::now();
        let client_clone: Arc<OnlineClient<SubstrateConfig>> = Arc::clone(&client);
        let signer_clone: Arc<PairSigner<SubstrateConfig, sr25519::Pair>> = Arc::clone(&signer);
        let result = tokio::spawn(async move {
            client_clone
                .tx()
                .sign_and_submit_then_watch(&payload, &*signer_clone, Default::default())
                .await
        })
        .await??;
        let sign_and_submit_duration = sign_and_submit_start.elapsed();
        info!("⏱️ sign_and_submit took {:?}", sign_and_submit_duration);

        // Wait for transaction finalization
        let finalization_start = Instant::now();
        match result.wait_for_finalized_success().await {
            Ok(events) => {
                let finalization_duration = finalization_start.elapsed();
                info!(
                    "⏱️ wait_for_finalized_success took {:?}",
                    finalization_duration
                );
                let block_hash: H256 = events.extrinsic_hash();
                info!(
                    "🎯 Registration successful at block {}. Events: {:?}",
                    block_hash, events
                );
                break; // Exit the loop on successful registration
            }
            Err(e) => {
                error!("Registration failed: {:?}", e);
                // Continue to next iteration
            }
        }

        // Implement rate limiting
        let mut last_attempt_guard = last_attempt.lock().await;
        if last_attempt_guard.elapsed() < Duration::from_secs(12) {
            tokio::time::sleep(Duration::from_secs(12) - last_attempt_guard.elapsed()).await;
        }
        *last_attempt_guard = Instant::now();
    }

    Ok(())
}

/// Retrieves the current recycle cost for a given network UID
///
/// # Arguments
///
/// * `client` - A reference to the blockchain client
/// * `netuid` - The network UID to check
///
/// # Returns
///
/// A `Result` containing the recycle cost as a `u64` if successful, or an `Err` if retrieval fails
async fn get_recycle_cost(
    client: &OnlineClient<SubstrateConfig>,
    netuid: u16,
) -> Result<u64, Box<dyn std::error::Error>> {
    let latest_block = client.blocks().at_latest().await?;
    let burn_key = subxt::storage::dynamic(
        "SubtensorModule",
        "Burn",
        vec![Value::primitive(scale_value::Primitive::U128(
            netuid as u128,
        ))],
    );
    let burn_cost: u64 = client
        .storage()
        .at(latest_block.hash())
        .fetch(&burn_key)
        .await?
        .ok_or_else(|| "Burn value not found for the given netuid".to_string())?
        .as_type::<u64>()?;

    Ok(burn_cost)
}

// TODO: Return UID of the registered neuron
/// Main function to run the registration script
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with INFO level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting registration script...");

    // Parse configuration parameters
    let params: RegistrationParams = parse_config()?;

    // Attempt to register hotkey
    if let Err(e) = register_hotkey(&params).await {
        error!("Error during registration: {}", e);
        return Err(e);
    }

    info!("Registration process completed successfully.");
    Ok(())
}
