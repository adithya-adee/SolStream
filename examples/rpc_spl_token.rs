//! SPL Token Transfer Indexer Example
//!
//! This example demonstrates indexing SPL token transfers by decoding instructions.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, config::BackfillConfig, EventDiscriminator, EventHandler,
    InstructionDecoder, SolanaIndexerConfigBuilder, SolanaIndexerError,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SplTransferEvent {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
}

impl EventDiscriminator for SplTransferEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("SplTransferEvent")
    }
}

pub struct SplTransferDecoder;
impl InstructionDecoder<SplTransferEvent> for SplTransferDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<SplTransferEvent> {
        if let UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) = instruction {
            if parsed.program != "spl-token" {
                return None;
            }
            let info = parsed.parsed.as_object()?.get("info")?.as_object()?;
            let instruction_type = parsed.parsed.as_object()?.get("type")?.as_str()?;
            if instruction_type != "transfer" && instruction_type != "transferChecked" {
                return None;
            }

            let amount = if instruction_type == "transferChecked" {
                info.get("tokenAmount")?
                    .as_object()?
                    .get("amount")?
                    .as_str()?
                    .parse::<u64>()
                    .ok()?
            } else {
                info.get("amount")?.as_str()?.parse::<u64>().ok()?
            };

            Some(SplTransferEvent {
                from: info.get("source")?.as_str()?.parse().ok()?,
                to: info.get("destination")?.as_str()?.parse().ok()?,
                amount,
            })
        } else {
            None
        }
    }
}

pub struct SplTransferHandler;
#[async_trait]
impl EventHandler<SplTransferEvent> for SplTransferHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS spl_transfers (
                signature TEXT PRIMARY KEY,
                from_wallet TEXT NOT NULL,
                to_wallet TEXT NOT NULL,
                amount BIGINT NOT NULL,
                indexed_at TIMESTAMPTZ DEFAULT NOW()
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    async fn handle(
        &self,
        event: SplTransferEvent,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        println!(
            "📝 SPL Transfer: {} -> {} ({} tokens)",
            event.from, event.to, event.amount
        );
        sqlx::query(
            "INSERT INTO spl_transfers (signature, from_wallet, to_wallet, amount)
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
    println!("🚀 SPL Token Transfer Indexer starting...");

    let rpc_url = std::env::var("RPC_URL")?;
    let database_url = std::env::var("DATABASE_URL")?;

    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(database_url.clone())
        .program_id(SPL_TOKEN_PROGRAM_ID)
        .with_backfill(BackfillConfig {
            enabled: true,
            ..Default::default()
        })
        .build()?;

    let mut indexer = solana_indexer_sdk::SolanaIndexer::new(config).await?;

    let handler = SplTransferHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&database_url).await?)
        .await?;

    indexer.register_decoder("spl-token", SplTransferDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting indexer...");
    indexer.start().await?;
    Ok(())
}
