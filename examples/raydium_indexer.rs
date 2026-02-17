//! Raydium AMM v4 Swap Indexer Example
//!
//! This example demonstrates how to decode partially decoded instructions to
//! index Raydium swaps.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, config::BackfillConfig, EventDiscriminator, EventHandler,
    InstructionDecoder, SolanaIndexer, SolanaIndexerConfigBuilder, SolanaIndexerError,
};
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

const RAYDIUM_V4_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RaydiumSwapEvent {
    pub amount_in: u64,
    pub min_amount_out: u64,
    pub user: String,
}

impl EventDiscriminator for RaydiumSwapEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("RaydiumSwapEvent")
    }
}

pub struct RaydiumSwapDecoder;
impl InstructionDecoder<RaydiumSwapEvent> for RaydiumSwapDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<RaydiumSwapEvent> {
        if let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(decoded)) = instruction {
            if decoded.program_id != RAYDIUM_V4_PROGRAM_ID {
                return None;
            }

            let data_bytes = solana_sdk::bs58::decode(&decoded.data).into_vec().ok()?;

            // Raydium SwapBaseIn Instruction is index 9 (formerly 3 in older versions)
            if data_bytes.is_empty() || data_bytes[0] != 9 {
                return None;
            }
            if data_bytes.len() < 17 {
                return None;
            }

            let amount_in = u64::from_le_bytes(data_bytes[1..9].try_into().ok()?);
            let min_amount_out = u64::from_le_bytes(data_bytes[9..17].try_into().ok()?);
            let user = decoded.accounts.first()?.clone();

            return Some(RaydiumSwapEvent {
                amount_in,
                min_amount_out,
                user,
            });
        }
        None
    }
}

pub struct RaydiumSwapHandler;
#[async_trait]
impl EventHandler<RaydiumSwapEvent> for RaydiumSwapHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS raydium_swaps (
                signature TEXT,
                user_wallet TEXT,
                amount_in BIGINT,
                min_amount_out BIGINT,
                PRIMARY KEY(signature, user_wallet)
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    async fn handle(
        &self,
        event: RaydiumSwapEvent,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "INSERT INTO raydium_swaps (signature, user_wallet, amount_in, min_amount_out)
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(&context.signature)
        .bind(&event.user)
        .bind(event.amount_in as i64)
        .bind(event.min_amount_out as i64)
        .execute(db)
        .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("🚀 Starting Raydium Indexer...");

    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let db_url = std::env::var("DATABASE_URL")?;

    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url.clone())
        .program_id(RAYDIUM_V4_PROGRAM_ID)
        .with_backfill(BackfillConfig {
            enabled: true,
            ..Default::default()
        })
        .build()?;

    let mut indexer = SolanaIndexer::new(config).await?;

    let handler = RaydiumSwapHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&db_url).await?)
        .await?;

    indexer.register_decoder(RAYDIUM_V4_PROGRAM_ID, RaydiumSwapDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting indexer...");
    indexer.start().await?;

    Ok(())
}
