//! SolStream example application.
//!
//! This demonstrates how to configure and run SolStream with environment variables.

#![warn(clippy::all, clippy::pedantic)]

use solana_indexer::{Poller, SolanaIndexerConfigBuilder};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv()?;

    // Retrieve configuration from environment variables
    let rpc_url = env::var("RPC_URL")?;
    let database_url = env::var("DATABASE_URL")?;
    let program_id = env::var("PROGRAM_ID")?;

    println!("Initializing SolStream...");
    println!("RPC URL: {}", rpc_url);
    println!("Program ID: {}", program_id);

    // Build configuration using the builder pattern
    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(database_url)
        .program_id(program_id)
        .with_poll_interval(5) // Poll every 5 seconds
        .with_batch_size(100) // Fetch up to 100 signatures per poll
        .build()?;

    println!("Configuration validated successfully!");
    println!("Poll interval: {} seconds", config.poll_interval_secs);
    println!("Batch size: {}", config.batch_size);

    // Create poller instance
    let mut poller = Poller::new(config);

    println!("\nStarting poller...");
    println!("Press Ctrl+C to stop");

    // Start polling (this runs indefinitely)
    poller.start().await?;

    Ok(())
}
