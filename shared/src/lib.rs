mod errors;

use crate::errors::Error;
use clap::Parser;
use log::{error, info, warn};
use serde::de::DeserializeOwned;
use std::fs;
use std::time::{Duration, Instant};
use subxt::{OnlineClient, SubstrateConfig};

/// Estimates the average block time of a Substrate-based blockchain.
///
/// This function samples a specified number of blocks and calculates the average time
/// between them. It also applies bounds to ensure the estimated time falls within
/// a reasonable range.
///
/// # Arguments
///
/// * `client` - A reference to an `OnlineClient<SubstrateConfig>` used to interact with the blockchain.
///
/// # Returns
///
/// * `Result<Duration, Box<dyn std::error::Error>>` - The estimated block time or an error.
///
/// # Example
///
/// ```
/// use subxt::OnlineClient;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = OnlineClient::<SubstrateConfig>::new().await?;
///     let estimated_time = estimate_block_time(&client).await?;
///     println!("Estimated block time: {:?}", estimated_time);
///     Ok(())
/// }
/// ```
pub async fn estimate_block_time(
    client: &OnlineClient<SubstrateConfig>,
) -> Result<Duration, Box<dyn std::error::Error>> {
    const BASE_BLOCK_TIME: Duration = Duration::from_secs(12);
    const MAX_BLOCK_TIME: Duration = Duration::from_secs(60);
    const SAMPLE_SIZE: u32 = 10;
    const MAX_WAIT_TIME: Duration = Duration::from_secs(120);

    let backend = client.backend();
    let start_time: Instant = Instant::now();

    let start_block_ref = backend.latest_finalized_block_ref().await?;
    let start_header = backend
        .block_header(start_block_ref.hash())
        .await?
        .ok_or_else(|| Error::Other("Start block header not found".into()))?;
    let start_block_number: u32 = start_header.number;
    let target_block_number: u32 = start_block_number + SAMPLE_SIZE;

    // Wait for SAMPLE_SIZE blocks to be produced
    while {
        let block_ref = backend.latest_finalized_block_ref().await?;
        let header = backend
            .block_header(block_ref.hash())
            .await?
            .ok_or_else(|| Error::Other("Block header not found".into()))?;
        header.number < target_block_number
    } {
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if we've exceeded the maximum wait time
        if start_time.elapsed() > MAX_WAIT_TIME {
            return Err("Exceeded maximum wait time for block sampling".into());
        }
    }

    let total_elapsed_time: Duration = start_time.elapsed();
    let average_block_time: Duration = total_elapsed_time / SAMPLE_SIZE;
    let estimated_block_time: Duration = average_block_time.clamp(BASE_BLOCK_TIME, MAX_BLOCK_TIME);

    if estimated_block_time == BASE_BLOCK_TIME || estimated_block_time == MAX_BLOCK_TIME {
        warn!(
            "Estimated block time ({:?}) hit the bounds. This might indicate unusual network conditions.",
            estimated_block_time
        );
    }

    info!(
        "Estimated block time: {:?} (based on {} block sample)",
        estimated_block_time, SAMPLE_SIZE
    );

    Ok(estimated_block_time)
}

// TODO: Consider implementing a more sophisticated estimation algorithm that accounts for
// network congestion and temporary fluctuations in block production times.

// TODO: Add unit tests to verify the function's behavior under various conditions,
// including simulated network delays and edge cases.

pub fn parse_config<T: Parser + DeserializeOwned>() -> Result<T, Box<dyn std::error::Error>> {
    if let Ok(config_str) = fs::read_to_string("config.toml") {
        info!("Found config.toml, parsing...");
        match toml::from_str(&config_str) {
            Ok(params) => {
                info!("Successfully parsed config.toml");
                Ok(params)
            }
            Err(e) => {
                error!("Error parsing config.toml: {}", e);
                Err(Box::new(e))
            }
        }
    } else {
        info!("No config.toml found, parsing command line arguments...");
        Ok(T::parse())
    }
}
