use clap::Parser;
use log::{error, info};
use subtensor_analysis::{AnalysisConfig, SubtensorAnalyzer};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    chain_endpoint: String,

    #[clap(long, default_value = "analysis_output")]
    output_dir: String,

    #[clap(long, default_value = "100")]
    num_blocks: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let cli = Cli::parse();

    let config = AnalysisConfig {
        chain_endpoint: cli.chain_endpoint,
        output_dir: cli.output_dir,
    };

    info!("Starting Subtensor analysis...");

    let analyzer = SubtensorAnalyzer::new(config).await?;

    info!("Collecting data for {} blocks...", cli.num_blocks);
    analyzer.collect_data(cli.num_blocks).await?;

    info!("Analyzing block times...");
    analyzer.analyze_block_times()?;

    info!("Analyzing optimal submission time...");
    let optimal_delay = analyzer.analyze_optimal_submission_time().await?;
    info!("Optimal submission delay: {:?}", optimal_delay);

    info!("Analysis complete.");

    Ok(())
}
