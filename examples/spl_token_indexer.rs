//! SPL Token Transfer Indexer Example
//!
//! Demonstrates how to build a general-purpose indexer for SPL token transfers.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer::{
    EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexerError,
    calculate_discriminator,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

/// SPL Token Transfer Event
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

/// Decoder for SPL token transfer instructions
pub struct SplTransferDecoder;

impl InstructionDecoder<SplTransferEvent> for SplTransferDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<SplTransferEvent> {
        match instruction {
            UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) => {
                // Check if this is an SPL token transfer
                if parsed.program != "spl-token" {
                    return None;
                }

                // Parse the instruction type
                let parsed_info = parsed.parsed.as_object()?;
                let instruction_type = parsed_info.get("type")?.as_str()?;

                if instruction_type != "transfer" && instruction_type != "transferChecked" {
                    return None;
                }

                // Extract transfer details
                let info = parsed_info.get("info")?.as_object()?;

                let source = info.get("source")?.as_str()?;
                let destination = info.get("destination")?.as_str()?;

                // Get amount - handle both transfer and transferChecked
                let amount = if instruction_type == "transferChecked" {
                    let token_amount = info.get("tokenAmount")?.as_object()?;
                    token_amount.get("amount")?.as_str()?.parse::<u64>().ok()?
                } else {
                    info.get("amount")?.as_str()?.parse::<u64>().ok()?
                };

                Some(SplTransferEvent {
                    from: source.parse().ok()?,
                    to: destination.parse().ok()?,
                    amount,
                })
            }
            _ => None,
        }
    }
}

/// Handler for SPL token transfer events
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
        db: &PgPool,
        signature: &str,
    ) -> Result<(), SolanaIndexerError> {
        println!(
            "Processing SPL Transfer: {} -> {} ({} tokens)",
            event.from, event.to, event.amount
        );

        sqlx::query(
            "INSERT INTO spl_transfers (signature, from_wallet, to_wallet, amount)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (signature) DO NOTHING",
        )
        .bind(signature)
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

    let rpc_url = std::env::var("RPC_URL")?;
    let _database_url = std::env::var("DATABASE_URL")?;

    println!("Starting SPL Token Transfer Indexer...");
    println!("RPC: {}", rpc_url);

    // Note: This is a simplified example showing the new architecture
    // The full integration with SolanaIndexer will be completed in the next phase
    println!("Example demonstrates InstructionDecoder<T> and EventHandler<T> patterns");
    println!("See implementation above for decoder and handler logic");

    Ok(())
}
