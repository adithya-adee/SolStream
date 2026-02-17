//! System Transfer Indexer Example (WebSocket)
//!
//! This example demonstrates real-time indexing of System Program transfers using a WebSocket connection.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, config::BackfillConfig, EventDiscriminator, EventHandler,
    InstructionDecoder, SolanaIndexer, SolanaIndexerConfigBuilder, SolanaIndexerError,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

// 1. Define Event, Decoder, and Handler
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
            "CREATE TABLE IF NOT EXISTS ws_system_transfers (
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
            "⚡ WS Transfer: {} -> {} ({} lamports)",
            event.from, event.to, event.amount
        );
        sqlx::query(
            "INSERT INTO ws_system_transfers (signature, from_wallet, to_wallet, amount_lamports)
             VALUES ($1, $2, $3, $4) ON CONFLICT (signature) DO NOTHING",
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
    println!("🚀 System Transfer Indexer (WebSocket) starting...");

    let ws_url = "wss://api.devnet.solana.com";
    let rpc_url = "https://api.devnet.solana.com";
    let database_url = std::env::var("DATABASE_URL")?;
    let program_id = "11111111111111111111111111111111";

    // 2. Build the indexer configuration.
    let config = SolanaIndexerConfigBuilder::new()
        .with_ws(ws_url, rpc_url)
        .with_database(database_url.clone())
        .program_id(program_id)
        .with_backfill(BackfillConfig {
            enabled: true,
            ..Default::default()
        })
        .build()?;

    let mut indexer = SolanaIndexer::new(config).await?;

    // 3. Register components.
    let handler = SystemTransferHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&database_url).await?)
        .await?;
    indexer.register_decoder("system", SystemTransferDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting WebSocket indexer...");

    // 4. Start the indexer.
    indexer.start().await?;

    Ok(())
}
