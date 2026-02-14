//! Solana Indexer - Example Usage
//!
//! This example demonstrates how to use the Solana Indexer SDK to index
//! transactions from a Solana program.

use solana_indexer_sdk::{Result, SolanaIndexer, SolanaIndexerConfigBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenvy::dotenv().ok();

    println!(
        "=== Solana Indexer Example ===
"
    );

    // Build configuration from environment variables
    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(std::env::var("RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".to_string()))
        .with_database(
            std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost/solana_indexer".to_string()),
        )
        .program_id(
            std::env::var("PROGRAM_ID")
                .unwrap_or_else(|_| "11111111111111111111111111111111".to_string()),
        )
        .with_poll_interval(5)
        .with_batch_size(10)
        .build()?;

    println!("Configuration:");
    println!("  RPC URL: {}", config.rpc_url());
    println!("  Database: {}", config.database_url);
    println!(
        "  Program IDs: {}",
        config
            .program_ids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    );
    println!("  Poll Interval: {} seconds", config.poll_interval_secs);
    println!(
        "  Batch Size: {}
",
        config.batch_size
    );

    // Create indexer
    let indexer = SolanaIndexer::new(config).await?;

    println!("Indexer initialized successfully!");
    println!("Database schema created.");
    println!(
        "
Starting indexer...
"
    );

    // Start indexing (runs indefinitely)
    indexer.start().await?;

    Ok(())
}
