//! Helius System Transfer Indexer Example
//!
//! This example demonstrates how to use Helius as a high-performance data source
//! for indexing System Program transfers.
//!
//! ## Usage
//!
//! Set environment variables in `.env`:
//! ```env
//! HELIUS_API_KEY=your_api_key
//! DATABASE_URL=postgresql://postgres:password@localhost/solana_indexer_sdk
//! ```

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator,
    config::{BackfillConfig, HeliusNetwork},
    EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexerConfigBuilder,
    SolanaIndexerError,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

// 1. Define Event, Decoder, and Handler (identical to other system_transfer examples)
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SystemTransferEvent {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

impl EventDiscriminator for SystemTransferEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("SystemTransferEvent")
    }
}

pub struct SystemTransferDecoder;
impl InstructionDecoder<SystemTransferEvent> for SystemTransferDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<SystemTransferEvent> {
        if let UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) = instruction {
            if parsed.program == "system" && parsed.parsed.get("type")?.as_str()? == "transfer" {
                let info = parsed.parsed.get("info")?.as_object()?;
                Some(SystemTransferEvent {
                    from: info.get("source")?.as_str()?.parse().ok()?,
                    to: info.get("destination")?.as_str()?.parse().ok()?,
                    amount: info.get("lamports")?.as_u64()?,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct SystemTransferHandler;
#[async_trait]
impl EventHandler<SystemTransferEvent> for SystemTransferHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS helius_system_transfers (
                signature TEXT PRIMARY KEY,
                from_wallet TEXT NOT NULL,
                to_wallet TEXT NOT NULL,
                amount_lamports BIGINT NOT NULL
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    async fn handle(
        &self,
        event: SystemTransferEvent,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        println!(
            "⚡ Helius Transfer: {} -> {} ({} lamports)",
            event.from, event.to, event.amount
        );

        sqlx::query(
            "INSERT INTO helius_system_transfers (signature, from_wallet, to_wallet, amount_lamports)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (signature) DO NOTHING",
        )
        .bind(&context.signature)
        .bind(event.from.to_string())
        .bind(event.to.to_string())
        .bind(event.amount as i64)
        .execute(db)
        .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("🚀 Helius System Transfer Indexer starting...");

    let api_key = std::env::var("HELIUS_API_KEY")?;
    let database_url = std::env::var("DATABASE_URL")?;
    let program_id = "11111111111111111111111111111111";

    let network = HeliusNetwork::Mainnet;

    // 2. Configure dynamic backfill.
    let backfill_config = BackfillConfig {
        enabled: true,
        ..Default::default()
    };

    // 3. Build the indexer configuration using Helius.
    let config = SolanaIndexerConfigBuilder::new()
        .with_helius_network(api_key, network, true) // `true` enables WebSocket streaming
        .with_database(database_url.clone())
        .program_id(program_id)
        .with_backfill(backfill_config)
        .build()?;

    let mut indexer = solana_indexer_sdk::SolanaIndexer::new(config).await?;

    // 4. Register components.
    let handler = SystemTransferHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&database_url).await?)
        .await?;

    indexer.register_decoder("system", SystemTransferDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting Helius indexer...");
    println!("   Press Ctrl+C to stop.");

    // 5. Start the indexer.
    indexer.start().await?;

    Ok(())
}
