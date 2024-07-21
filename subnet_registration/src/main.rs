use clap::Parser;
use log::{error, info};
use serde::Deserialize;
use shared::estimate_block_time;
use subxt::ext::scale_encode::Composite;
// use subxt::ext::sp_core::Pair;
// use sp_core::sr25519;
// use sp_core::Pair;
use std::sync::Arc;
use std::time::{Duration, Instant};
use subxt::ext::sp_core::{sr25519, Pair};
use subxt::tx::{DefaultPayload, PairSigner, Payload};
use subxt::utils::Static;
use subxt::{OnlineClient, SubstrateConfig};
use tokio::sync::Mutex;

// use subxt::ext::sp_core::sr25519;
// use subxt::ext::sp_core::sr25519::Pair;

#[derive(Parser, Deserialize, Debug)]
#[clap(author, version, about, long_about = None)]
struct SubnetRegistrationParams {
    #[clap(long)]
    coldkey: String,

    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    chain_endpoint: String,
}

// use subxt::{tx::Payload, utils::Static};

async fn register_subnet(
    params: &SubnetRegistrationParams,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(OnlineClient::<SubstrateConfig>::from_url(&params.chain_endpoint).await?);

    let coldkey: sr25519::Pair =
        Pair::from_string(&params.coldkey, None).map_err(|_| "Invalid coldkey")?;
    let signer = PairSigner::new(coldkey.clone());

    // Monitor pending extrinsics
    tokio::spawn(monitor_pending_extrinsics(Arc::clone(&client)));

    // Estimate block time
    let estimated_block_time = estimate_block_time(&client).await?;
    info!("Estimated block time: {:?}", estimated_block_time);

    // Prepare and submit the register_subnet extrinsic
    let call = subxt::tx::Payload::new("SubtensorModule", "register_network", ()).into_static();

    loop {
        match client
            .tx()
            .sign_and_submit_then_watch(&call, &signer, Default::default())
            .await
        {
            Ok(result) => match result.wait_for_finalized_success().await {
                Ok(events) => {
                    info!("Subnet registration successful. Events: {:?}", events);
                    break;
                }
                Err(e) => error!("Subnet registration failed: {:?}", e),
            },
            Err(e) => error!("Failed to submit extrinsic: {:?}", e),
        }
        tokio::time::sleep(estimated_block_time).await;
    }

    Ok(())
}

async fn monitor_pending_extrinsics(client: Arc<OnlineClient<SubstrateConfig>>) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        match client.rpc().pending_extrinsics().await {
            Ok(pending_extrinsics) => {
                info!("Number of pending extrinsics: {}", pending_extrinsics.len());
                for (index, extrinsic) in pending_extrinsics.iter().enumerate() {
                    info!("Extrinsic {}: {:?}", index, extrinsic);
                }
            }
            Err(e) => error!("Failed to fetch pending extrinsics: {:?}", e),
        }
    }
}

// async fn estimate_block_time(
//     client: &OnlineClient<SubstrateConfig>,
// ) -> Result<Duration, Box<dyn std::error::Error>> {
//     let start_block = client.blocks().at_latest().await?.number();
//     tokio::time::sleep(Duration::from_secs(60)).await;
//     let end_block = client.blocks().at_latest().await?.number();
//     let blocks_passed = end_block - start_block;
//     Ok(Duration::from_secs(60) / blocks_passed)
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let params = SubnetRegistrationParams::parse();
    register_subnet(&params).await
}
