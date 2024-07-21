use clap::Parser;
use log::{debug, info, warn};
use serde::Deserialize;
use shared::parse_config;
use std::time::Duration;
use subtensor_analysis::{AnalysisConfig, SubtensorAnalyzer};
use tokio::time::sleep;

#[derive(Parser, Debug, Deserialize)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    chain_endpoint: String,

    #[clap(long, default_value = "analysis_output")]
    output_dir: String,

    #[clap(long, default_value = "100")]
    num_blocks: u32,

    #[clap(long)]
    coldkey: Option<String>,

    #[clap(long)]
    hotkey: Option<String>,

    #[clap(long)]
    netuid: Option<u16>,

    #[clap(long)]
    max_cost: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cli: Cli = parse_config()?;

    println!("Parsed configuration:");
    println!("Chain Endpoint: {}", cli.chain_endpoint);
    println!("Output Directory: {}", cli.output_dir);
    println!("Number of Blocks: {}", cli.num_blocks);

    let config = AnalysisConfig {
        chain_endpoint: cli.chain_endpoint,
        output_dir: cli.output_dir,
    };
    debug!("Created AnalysisConfig");

    info!("Starting Subtensor analysis...");

    let max_retries = 5;
    let mut retry_count = 0;
    let mut analyzer = None;

    while retry_count < max_retries {
        match SubtensorAnalyzer::new(config.clone()).await {
            Ok(a) => {
                analyzer = Some(a);
                break;
            }
            Err(e) => {
                warn!(
                    "Failed to connect to the blockchain: {}. Retrying in 5 seconds...",
                    e
                );
                retry_count += 1;
                sleep(Duration::from_secs(5)).await;
            }
        }
    }

    // let analyzer = SubtensorAnalyzer::new(config).await?;

    info!("Collecting data for {} blocks...", cli.num_blocks);
    analyzer
        .clone()
        .unwrap()
        .collect_data(cli.num_blocks)
        .await?;

    info!("Analyzing block times...");
    analyzer.clone().unwrap().analyze_block_times()?;

    info!("Analyzing optimal submission time...");
    let optimal_delay = analyzer
        .clone()
        .unwrap()
        .analyze_optimal_submission_time()
        .await?;
    info!("Optimal submission delay: {:?}", optimal_delay);

    info!("Analysis complete.");

    Ok(())
}
