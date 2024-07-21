//! A crate for analyzing Subtensor blockchain data, focusing on block times and transaction placement.
//!
//! This crate provides functionality to:
//! - Interact with the Subtensor blockchain
//! - Collect data on block times, pending extrinsics, and block weight
//! - Store the collected data in Parquet files
//! - Analyze the data using Polars for optimal transaction timing
//!
//! # Example
//!
//! ```rust,no_run
//! use subtensor_analysis::{SubtensorAnalyzer, AnalysisConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = AnalysisConfig {
//!         chain_endpoint: "ws://127.0.0.1:9944".to_string(),
//!         output_dir: "analysis_output".to_string(),
//!     };
//!     
//!     let analyzer = SubtensorAnalyzer::new(config).await?;
//!     analyzer.collect_data(100).await?; // Collect data for 100 blocks
//!     analyzer.analyze_block_times()?;
//!     
//!     Ok(())
//! }
//! ```

use chrono::Utc;
use futures::StreamExt;
use log::{error, info};
use polars::prelude::*;
use shared::estimate_block_time;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use subxt::backend::rpc::{rpc_params, RpcClient};
use subxt::dynamic::Value;
use subxt::ext::scale_value::{Composite, ValueDef};
use subxt::utils::H256;
use subxt::{OnlineClient, SubstrateConfig};

/// Configuration for the Subtensor analysis
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    /// The WebSocket endpoint for the Subtensor blockchain
    pub chain_endpoint: String,
    /// The directory to store output Parquet files
    pub output_dir: String,
}

/// Main struct for analyzing Subtensor blockchain data

pub struct SubtensorAnalyzer {
    client: OnlineClient<SubstrateConfig>,
    rpc_client: RpcClient,
    output_dir: PathBuf,
}

impl SubtensorAnalyzer {
    /// Create a new SubtensorAnalyzer instance
    pub async fn new(config: AnalysisConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let client = OnlineClient::<SubstrateConfig>::from_url(&config.chain_endpoint).await?;
        let rpc_client = RpcClient::from_url(&config.chain_endpoint).await?;
        let output_dir = PathBuf::from(config.output_dir);

        std::fs::create_dir_all(&output_dir)?;

        Ok(Self {
            client,
            rpc_client,
            output_dir,
        })
    }

    /// Collect blockchain data for a specified number of blocks
    pub async fn collect_data(&self, num_blocks: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut block_times = Vec::new();
        let mut pending_extrinsics_counts = Vec::new();
        let mut block_weights = Vec::new();

        let mut blocks = self.client.blocks().subscribe_finalized().await?;
        let start_time = Instant::now();

        for _ in 0..num_blocks {
            if let Some(block) = blocks.next().await {
                let block = block?;
                let block_number = block.header().number;

                // Collect block time
                let block_time = start_time.elapsed();
                block_times.push(block_time.as_secs_f64());

                // Collect pending extrinsics count
                let pending_extrinsics: Vec<String> = self
                    .rpc_client
                    .request("author_pendingExtrinsics", rpc_params![])
                    .await?;
                pending_extrinsics_counts.push(pending_extrinsics.len() as u32);

                // Collect block weight (you may need to adjust this based on Subtensor's specific API)
                let block_weight = self.get_block_weight(block_number).await?;
                block_weights.push(block_weight);

                info!("Collected data for block {}", block_number);
            }
        }

        // Create a DataFrame and save to Parquet
        let output_file = self.output_dir.join("block_data.parquet");
        let mut df = df!(
            "block_time" => block_times,
            "pending_extrinsics" => pending_extrinsics_counts,
            "block_weight" => block_weights
        )?;
        ParquetWriter::new(std::fs::File::create(output_file)?).finish(&mut df)?;

        Ok(())
    }

    /// Analyze block times and provide insights
    pub fn analyze_block_times(&self) -> Result<(), Box<dyn std::error::Error>> {
        let input_file = self.output_dir.join("block_data.parquet");
        let df = ParquetReader::new(std::fs::File::open(input_file)?).finish()?;

        // Calculate average block time
        let avg_block_time: f64 = df.column("block_time")?.mean().unwrap();

        // In analyze_block_times function:
        let corr = calculate_correlation(&df, "pending_extrinsics", "block_time")?;
        let weight_impact = calculate_correlation(&df, "block_weight", "block_time")?;

        info!("Average block time: {:.2} seconds", avg_block_time);
        info!(
            "Correlation between pending extrinsics and block time: {:.2}",
            corr
        );
        info!("Impact of block weight on block time: {:.2}", weight_impact);

        Ok(())
    }

    /// Get the block weight for a given block number
    ///
    /// This function retrieves the block weight for a specified block number from the Subtensor blockchain.
    /// The block weight is a measure of the computational resources required to process the block.
    ///
    /// # Arguments
    ///
    /// * `block_number` - A u32 representing the block number to query
    ///
    /// # Returns
    ///
    /// A Result containing the block weight as a u64, or an error if the retrieval fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use subtensor_analysis::{SubtensorAnalyzer, AnalysisConfig};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = AnalysisConfig {
    /// #     chain_endpoint: "ws://127.0.0.1:9944".to_string(),
    /// #     output_dir: "analysis_output".to_string(),
    /// # };
    /// let analyzer = SubtensorAnalyzer::new(config).await?;
    /// let block_weight = analyzer.get_block_weight(12345).await?;
    /// println!("Block weight: {}", block_weight);
    /// # Ok(())
    /// # }
    /// ```
    async fn get_block_weight(&self, block_number: u32) -> Result<u64, Box<dyn std::error::Error>> {
        // Convert block number to SCALE-encoded bytes
        let block_number_bytes = block_number.to_le_bytes();

        // RPC call to get the block hash for the given block number
        let block_hash: H256 = self
            .rpc_client
            .request("chain_getBlockHash", rpc_params![block_number])
            .await?;

        // RPC call to get the block header
        let header: Value = self
            .rpc_client
            .request("chain_getHeader", rpc_params![block_hash])
            .await?;

        // Extract the extrinsics root from the header
        let extrinsics_root = match &header.value {
            ValueDef::Composite(Composite::Named(fields)) => fields
                .iter()
                .find(|(k, _)| k == "extrinsicsRoot")
                .and_then(|(_, v)| v.as_str()),
            _ => None,
        }
        .ok_or("Failed to extract extrinsics root")?;

        // RPC call to get the block weight
        let weight: Value = self
            .rpc_client
            .request(
                "system_queryStorageAt",
                rpc_params!["System", "BlockWeight", extrinsics_root, block_number_bytes],
            )
            .await?;

        // Extract the total weight from the returned value
        let total_weight = match &weight.value {
            ValueDef::Composite(Composite::Unnamed(arr)) if !arr.is_empty() => {
                match &arr[0].value {
                    ValueDef::Composite(Composite::Named(fields)) => fields
                        .iter()
                        .find(|(k, _)| k == "total")
                        .and_then(|(_, v)| v.as_u128()),
                    _ => None,
                }
            }
            _ => None,
        }
        .ok_or("Failed to extract total weight")?;

        Ok(total_weight.try_into()?)
    }

    /// Analyze the optimal time to submit a transaction
    pub async fn analyze_optimal_submission_time(
        &self,
    ) -> Result<Duration, Box<dyn std::error::Error>> {
        let estimated_block_time = estimate_block_time(&self.client).await?;
        let input_file = self.output_dir.join("block_data.parquet");
        let df = ParquetReader::new(std::fs::File::open(input_file)?).finish()?;

        // Calculate the average number of pending extrinsics
        let avg_pending_extrinsics: f64 = df.column("pending_extrinsics")?.mean().unwrap();

        // Simple heuristic: submit when pending extrinsics are below average
        let optimal_delay = if avg_pending_extrinsics > 0.0 {
            estimated_block_time.mul_f64(0.5)
        } else {
            Duration::from_secs(0)
        };

        info!("Optimal submission delay: {:?}", optimal_delay);
        Ok(optimal_delay)
    }
}

// TODO: Implement additional analysis functions for optimal transaction timing
// TODO: Add more sophisticated statistical analysis or machine learning techniques
// TODO: Implement real-time monitoring of network conditions
// TODO: Create visualizations of the collected data

/// Calculates the Pearson correlation coefficient between two columns in a DataFrame.
///
/// # Arguments
///
/// * `df` - A reference to the DataFrame containing the data.
/// * `col1` - The name of the first column.
/// * `col2` - The name of the second column.
///
/// # Returns
///
/// A Result containing the correlation coefficient as an f64, or a PolarsError if the calculation fails.
///
/// # Example
///
/// ```
/// use polars::prelude::*;
/// use your_crate::calculate_correlation;
///
/// let df = DataFrame::new(vec![
///     Series::new("A", &[1.0, 2.0, 3.0, 4.0, 5.0]),
///     Series::new("B", &[1.0, 2.0, 3.0, 4.0, 5.0]),
/// ]).unwrap();
///
/// let correlation = calculate_correlation(&df, "A", "B").unwrap();
/// assert_eq!(correlation, 1.0);
/// ```
fn calculate_correlation(df: &DataFrame, col1: &str, col2: &str) -> Result<f64, PolarsError> {
    // Cast columns to Float64 to ensure compatibility
    let x: Series = df.column(col1)?.cast(&DataType::Float64)?;
    let y: Series = df.column(col2)?.cast(&DataType::Float64)?;

    // Calculate means
    let x_mean: f64 = x
        .mean()
        .ok_or_else(|| PolarsError::ComputeError("Failed to calculate mean for x".into()))?;
    let y_mean: f64 = y
        .mean()
        .ok_or_else(|| PolarsError::ComputeError("Failed to calculate mean for y".into()))?;

    // Calculate numerator and variances
    let mut numerator: f64 = 0.0;
    let mut x_variance: f64 = 0.0;
    let mut y_variance: f64 = 0.0;

    // Iterate over both series simultaneously
    for (x_val, y_val) in x.f64()?.into_iter().zip(y.f64()?.into_iter()) {
        if let (Some(x_val), Some(y_val)) = (x_val, y_val) {
            let x_diff: f64 = x_val - x_mean;
            let y_diff: f64 = y_val - y_mean;
            numerator += x_diff * y_diff;
            x_variance += x_diff.powi(2);
            y_variance += y_diff.powi(2);
        }
        // TODO: Consider how to handle None values. Current implementation skips them.
    }

    // Calculate and return the correlation coefficient
    Ok(numerator / (x_variance * y_variance).sqrt())
}
