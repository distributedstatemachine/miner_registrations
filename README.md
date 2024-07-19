# Miner Registration Script

A subnet registration script for the Bittensor written in Rust.

## Purpose

The registration script automates the process of registering new nodes on the a Bittensor subnet . It continually reads the chain storage , for the burn price , and then submits the transaction once the price is below the user's ,ax price

## Performance

This Rust implementation significantly outperforms the Python-based Substrate library:

- Python Substrate:
  - Recycle cost calculation time: 2.2478 seconds

- Rust Implementation:
  - Recycle cost calculation time: 659.007833ms (approximately 3.4x faster)

## Prerequisites

- Rust 1.70.0 or later
- Bankai node binary
- Access to a running Bankai network

## Usage

1. Clone the repository:
   ```
   git clone https://github.com/distributedstatemachine/miner_registrations.git
   ```

2. Update the config.toml file with your registration information.

3. Build the script:
   ```
   cargo build --release
   ```

4. Run the registration script:
   ```
   ./target/release/reg_script [OPTIONS]
   ```

   For available options, run:
   ```
   ./target/release/reg_script --help
   ```

## Configuration

Modify the `config.toml` file to adjust registration parameters such as:

- Coldkey and hotkey for registration
- Network UID
- Maximum registration cost
- Chain endpoint URL

See `config.example.toml` for an example configuration file.

 Do not commit it to the repo, as it contains your keys. `.gitignore` it. 

