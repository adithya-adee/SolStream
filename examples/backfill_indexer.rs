//! Dynamic Backfill Indexer Example
//!
//! This example demonstrates how the SDK automatically backfills historical data
//! while simultaneously processing live transactions.
//!
//! 1. **Dynamic Configuration**: Backfill is enabled without hardcoded slot ranges.
//! 2. **Automatic Operation**: A single `indexer.start()` call manages both live indexing
//!    and the background backfill process.
//! 3. **Complete Example**: A functional System Program transfer indexer is used to
//!    provide a real-world demonstration.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator,
    config::{BackfillConfig, StartStrategy},
    EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexer,
    SolanaIndexerConfigBuilder, SolanaIndexerError, TxMetadata,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

// 1. Define the event, decoder, and handler.
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
                return Some(SystemTransferEvent {
                    from: info.get("source")?.as_str()?.parse().ok()?,
                    to: info.get("destination")?.as_str()?.parse().ok()?,
                    amount: info.get("lamports")?.as_u64()?,
                });
            }
        }
        None
    }
}

pub struct SystemTransferHandler;
#[async_trait]
impl EventHandler<SystemTransferEvent> for SystemTransferHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS backfill_transfers (
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
        context: &TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "INSERT INTO backfill_transfers (signature, from_wallet, to_wallet, amount_lamports)
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
    println!("🚀 Starting Indexer with Dynamic Backfill...");

    let rpc_url = "https://api.devnet.solana.com".to_string();
    let database_url = std::env::var("DATABASE_URL")?;
    let program_id = "11111111111111111111111111111111";

    // 2. Configure dynamic backfill.
    // The indexer will automatically backfill if it is more than 1000 slots behind
    // the chain tip, checking for new ranges to fill every 10 seconds.
    let backfill_config = BackfillConfig {
        enabled: true,
        poll_interval_secs: 10,
        desired_lag_slots: Some(1000),
        ..Default::default()
    };

    // 3. Build the main indexer configuration.
    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(database_url.clone())
        .program_id(program_id)
        .with_start_strategy(StartStrategy::Resume)
        .with_backfill(backfill_config)
        .build()?;

    let mut indexer = SolanaIndexer::new(config).await?;

    // 4. Register components and initialize schema.
    let handler = SystemTransferHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&database_url).await?)
        .await?;

    indexer.register_decoder("system", SystemTransferDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting indexer.");
    println!("   The indexer will process live data and run backfill in the background.");
    println!("   Press Ctrl+C to stop.");

    // 5. Start the indexer.
    indexer.start().await?;

    Ok(())
}
