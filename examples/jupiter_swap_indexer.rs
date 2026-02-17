//! Jupiter Swap Indexer - Production Ready
//!
//! This example demonstrates indexing Jupiter v6 swaps by analyzing token balance changes.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, config::BackfillConfig, types::traits::InstructionDecoder,
    EventDiscriminator, EventHandler, SolanaIndexerConfigBuilder, SolanaIndexerError, TxMetadata,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::UiInstruction;
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct JupiterSwapEvent {
    pub route: String,
}

impl EventDiscriminator for JupiterSwapEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("JupiterSwapEvent")
    }
}

pub struct JupiterInstructionDecoder;

impl InstructionDecoder<JupiterSwapEvent> for JupiterInstructionDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<JupiterSwapEvent> {
        let _program_id = match instruction {
            UiInstruction::Compiled(c) => &c.program_id_index,
            UiInstruction::Parsed(solana_transaction_status::UiParsedInstruction::Parsed(p)) => {
                return if p
                    .program_id
                    .contains("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4")
                {
                    Some(JupiterSwapEvent {
                        route: "Jupiter v6".to_string(),
                    })
                } else {
                    None
                }
            }
            UiInstruction::Parsed(
                solana_transaction_status::UiParsedInstruction::PartiallyDecoded(p),
            ) => {
                return if p
                    .program_id
                    .contains("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4")
                {
                    Some(JupiterSwapEvent {
                        route: "Jupiter v6".to_string(),
                    })
                } else {
                    None
                }
            }
        };
        None
    }
}

pub struct JupiterSwapHandler;

impl JupiterSwapHandler {
    fn extract_transfers(
        &self,
        context: &TxMetadata,
    ) -> (Pubkey, Vec<(Pubkey, i64, String, String)>) {
        let mut changes = HashMap::new();
        let mut transfers = Vec::new();
        let mut user_wallet = Pubkey::default();

        for balance in &context.pre_token_balances {
            if let Ok(amount) = balance.amount.parse::<u64>() {
                changes.insert((balance.mint.clone(), balance.owner.clone()), amount as i64);
            }
        }

        for balance in &context.post_token_balances {
            if let Ok(amount) = balance.amount.parse::<u64>() {
                if let Some(pre_amount) =
                    changes.get_mut(&(balance.mint.clone(), balance.owner.clone()))
                {
                    *pre_amount -= amount as i64;
                } else {
                    changes.insert(
                        (balance.mint.clone(), balance.owner.clone()),
                        -(amount as i64),
                    );
                }
            }
        }

        for ((mint_str, owner_str), diff) in changes {
            if diff == 0 {
                continue;
            }

            if let (Ok(mint), Ok(owner)) =
                (Pubkey::from_str(&mint_str), Pubkey::from_str(&owner_str))
            {
                let direction = if diff > 0 { "out" } else { "in" };
                let abs_amount = diff.abs();

                transfers.push((mint, abs_amount, direction.to_string(), owner_str.clone()));

                if user_wallet == Pubkey::default() && !owner_str.contains("1111111111111111") {
                    user_wallet = owner;
                }
            }
        }
        (user_wallet, transfers)
    }
}

#[async_trait]
impl EventHandler<JupiterSwapEvent> for JupiterSwapHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query("DROP TABLE IF EXISTS jupiter_swap_transfers")
            .execute(db)
            .await?;
        sqlx::query("DROP TABLE IF EXISTS jupiter_swap_transactions CASCADE")
            .execute(db)
            .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jupiter_swap_transactions (
                signature TEXT PRIMARY KEY,
                slot BIGINT NOT NULL,
                block_time BIGINT,
                user_wallet TEXT NOT NULL,
                route TEXT NOT NULL,
                fee_lamports BIGINT,
                indexed_at TIMESTAMPTZ DEFAULT NOW()
            )",
        )
        .execute(db)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jupiter_swap_transfers (
                id SERIAL PRIMARY KEY,
                signature TEXT NOT NULL REFERENCES jupiter_swap_transactions(signature) ON DELETE CASCADE,
                mint TEXT NOT NULL,
                owner TEXT NOT NULL,
                amount BIGINT NOT NULL,
                direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
                indexed_at TIMESTAMPTZ DEFAULT NOW()
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }

    async fn handle(
        &self,
        event: JupiterSwapEvent,
        context: &TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let (user_wallet, transfers) = self.extract_transfers(context);
        if transfers.is_empty() {
            return Ok(());
        }

        let mut tx = db.begin().await?;
        sqlx::query(
            "INSERT INTO jupiter_swap_transactions
             (signature, slot, block_time, user_wallet, route, fee_lamports)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (signature) DO NOTHING",
        )
        .bind(&context.signature)
        .bind(context.slot as i64)
        .bind(user_wallet.to_string())
        .bind(&event.route)
        .bind(context.fee as i64)
        .execute(&mut *tx)
        .await?;

        for (mint, amount, direction, owner) in transfers {
            sqlx::query(
                "INSERT INTO jupiter_swap_transfers
                 (signature, mint, owner, amount, direction)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(&context.signature)
            .bind(mint.to_string())
            .bind(owner)
            .bind(amount)
            .bind(&direction)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("🚀 Jupiter Swap Indexer starting...");

    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let database_url = std::env::var("DATABASE_URL")?;
    let jupiter_program_id = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(database_url.clone())
        .program_id(jupiter_program_id)
        .with_backfill(BackfillConfig {
            enabled: true,
            ..Default::default()
        })
        .build()?;

    let mut indexer = solana_indexer_sdk::SolanaIndexer::new(config).await?;

    let handler = JupiterSwapHandler;
    handler
        .initialize_schema(&sqlx::PgPool::connect(&database_url).await?)
        .await?;

    indexer.register_decoder(jupiter_program_id, JupiterInstructionDecoder)?;
    indexer.register_handler(handler)?;

    println!("✅ Setup complete. Starting indexer...");
    indexer.start().await?;

    Ok(())
}
