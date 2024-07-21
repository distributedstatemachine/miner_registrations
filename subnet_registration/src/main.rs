use env_logger;
use shared::parse_config;
use subnet_registration::{register_subnet, SubnetRegistrationParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let params: SubnetRegistrationParams = parse_config()?;
    register_subnet(&params).await
}
