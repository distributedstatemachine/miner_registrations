use clap::Parser;
use log::{error, info};
use serde::Deserialize;
use shared::estimate_block_time;
use std::sync::Arc;
use std::time::Duration;
use subxt::backend::rpc::RpcClient;
use subxt::ext::sp_core::{sr25519, Pair};
use subxt::rpc_params;
use subxt::{OnlineClient, SubstrateConfig};
mod errors;
use crate::errors::Error;

#[subxt::subxt(runtime_metadata_path = "metadata.scale")]
pub mod runtime {}

#[derive(Parser, Deserialize, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct SubnetRegistrationParams {
    #[clap(long)]
    pub coldkey: String,

    #[clap(long, default_value = "ws://127.0.0.1:9944")]
    pub chain_endpoint: String,
}

pub async fn register_subnet(
    params: &SubnetRegistrationParams,
) -> Result<(), Box<dyn std::error::Error>> {
    let chain_endpoint = params.chain_endpoint.clone();

    let client = Arc::new(OnlineClient::<SubstrateConfig>::from_url(&params.chain_endpoint).await?);

    let coldkey: sr25519::Pair =
        Pair::from_string(&params.coldkey, None).map_err(|_| "Invalid coldkey")?;
    let signer = subxt::tx::PairSigner::new(coldkey);

    tokio::spawn(async move {
        if let Err(e) = monitor_pending_extrinsics(&chain_endpoint).await {
            error!("Error in monitor_pending_extrinsics: {:?}", e);
        }
    });

    let estimated_block_time = estimate_block_time(&client).await?;
    info!("Estimated block time: {:?}", estimated_block_time);

    let call = runtime::tx().subtensor_module().register_network();

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

async fn monitor_pending_extrinsics(chain_endpoint: &str) -> Result<(), Error> {
    let rpc = RpcClient::from_url(chain_endpoint)
        .await
        .map_err(|e| Error::ConnectionError(e.to_string()))?;
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        interval.tick().await;
        match rpc.request("author_pendingExtrinsics", rpc_params![]).await {
            Ok(pending_extrinsics) => {
                let extrinsics: Vec<String> = pending_extrinsics;
                info!("Number of pending extrinsics: {}", extrinsics.len());
                for (index, extrinsic) in extrinsics.iter().enumerate() {
                    info!("Extrinsic {}: {}", index, extrinsic);
                }
            }
            Err(e) => error!("Failed to fetch pending extrinsics: {:?}", e),
        }
    }
}
