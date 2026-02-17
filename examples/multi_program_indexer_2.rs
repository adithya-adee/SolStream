use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::config::BackfillConfig;
use solana_indexer_sdk::{
    calculate_discriminator, EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexer,
    SolanaIndexerConfigBuilder, SolanaIndexerError, Storage, TxMetadata,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

// ================================================================================================
// CONSTANTS
// ================================================================================================

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const JUPITER_PROGRAM_ID: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

// ================================================================================================
// EVENT: JUPITER SWAP
// ================================================================================================

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
        match instruction {
            UiInstruction::Compiled(compiled) => {
                if compiled.accounts.len() < 4 {
                    return None;
                }
                Some(JupiterSwapEvent {
                    route: "Jupiter v6".to_string(),
                })
            }
            UiInstruction::Parsed(parsed) => match parsed {
                UiParsedInstruction::Parsed(p) => {
                    if p.program == "jupiter" || p.program_id.contains("JUP") {
                        Some(JupiterSwapEvent {
                            route: "Jupiter v6".to_string(),
                        })
                    } else {
                        None
                    }
                }
                UiParsedInstruction::PartiallyDecoded(p) => {
                    if p.program_id.contains("JUP") {
                        Some(JupiterSwapEvent {
                            route: "Jupiter v6".to_string(),
                        })
                    } else {
                        None
                    }
                }
            },
        }
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
        println!("📊 Initializing Jupiter Swap Schema (Pre/Post Balance Analysis)");

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
            "CREATE INDEX IF NOT EXISTS idx_jup_tx_user ON jupiter_swap_transactions(user_wallet)",
        )
        .execute(db)
        .await?;

        sqlx::query("CREATE TABLE IF NOT EXISTS jupiter_swap_transfers (
            id SERIAL PRIMARY KEY,
            signature TEXT NOT NULL REFERENCES jupiter_swap_transactions(signature) ON DELETE CASCADE,
            mint TEXT NOT NULL,
            owner TEXT NOT NULL,
            amount BIGINT NOT NULL,
            direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
            indexed_at TIMESTAMPTZ DEFAULT NOW()
        )").execute(db).await?;

        Ok(())
    }

    async fn handle(
        &self,
        event: JupiterSwapEvent,
        context: &TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;

        // Idempotency check handled via INSERT ON CONFLICT DO NOTHING usually sufficient if signature is PK
        // But let's check explicitly if we want to avoid extra processing logic
        let exists: (i64,) =
            sqlx::query_as("SELECT 1 FROM jupiter_swap_transactions WHERE signature = $1")
                .bind(signature)
                .fetch_one(db)
                .await
                .unwrap_or((0,));

        if exists.0 == 1 {
            return Ok(());
        }

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
        .bind(signature)
        .bind(context.slot as i64)
        .bind(user_wallet.to_string())
        .bind(&event.route)
        .bind(context.fee as i64)
        .execute(&mut *tx)
        .await?;

        println!(
            "🔥 [Jupiter] Swap Indexed: {} | User: {} | Transfers: {}",
            &signature[..8],
            &user_wallet.to_string()[..8],
            transfers.len()
        );

        for (mint, amount, direction, owner) in transfers {
            sqlx::query(
                "INSERT INTO jupiter_swap_transfers 
                 (signature, mint, owner, amount, direction)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(signature)
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

// ================================================================================================
// EVENT: SYSTEM TRANSFER
// ================================================================================================

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SystemTransferEvent {
    pub from: String,
    pub to: String,
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
        match instruction {
            UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) => {
                if parsed.program_id != SYSTEM_PROGRAM_ID {
                    return None;
                }
                if parsed.parsed.get("type")?.as_str()? != "transfer" {
                    return None;
                }
                let info = parsed.parsed.get("info")?;
                let amount = info.get("lamports")?.as_u64()?;
                let from = info.get("source")?.as_str()?.to_string();
                let to = info.get("destination")?.as_str()?.to_string();

                Some(SystemTransferEvent { from, to, amount })
            }
            _ => None,
        }
    }
}

pub struct SystemTransferHandler;

#[async_trait]
impl EventHandler<SystemTransferEvent> for SystemTransferHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        println!("📊 Initializing System Transfer Schema");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_transfers (
            signature TEXT PRIMARY KEY,
            slot BIGINT NOT NULL,
            block_time BIGINT,
            from_address TEXT NOT NULL,
            to_address TEXT NOT NULL,
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
        event: SystemTransferEvent,
        context: &TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;

        sqlx::query(
            "INSERT INTO system_transfers (signature, slot, block_time, from_address, to_address, amount)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (signature) DO NOTHING"
        )
        .bind(signature)
        .bind(context.slot as i64)
        .bind(&event.from)
        .bind(&event.to)
        .bind(event.amount as i64)
        .execute(db)
        .await?;

        println!(
            "💸 [System] Transfer: {} -> {} ({} lamports) | Sig: {:.8}...",
            event.from, event.to, event.amount, signature
        );
        Ok(())
    }
}

// ================================================================================================
// MAIN
// ================================================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Logging
    dotenvy::dotenv().ok();
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    println!("🚀 Starting Multi-Program Indexer with Dynamic Backfill (Jupiter + System)...");

    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    println!("RPC: {}", rpc_url);
    println!("DB:  {}", db_url);

    // 1. Shared Storage
    println!("Initializing shared storage...");
    let storage = Arc::new(Storage::new(&db_url).await?);
    storage.initialize().await?;

    // 2. Dynamic Backfill Configuration
    println!("ℹ️ Dynamic backfill enabled. The indexer will backfill missing slots if behind the chain tip.");
    let jupiter_backfill_config = BackfillConfig {
        enabled: true,
        start_slot: None, // Let the trigger decide
        end_slot: None,   // Let the trigger decide
        batch_size: 100,
        concurrency: 10,
        enable_reorg_handling: true,
        finalization_check_interval: 100,
        poll_interval_secs: 10,        // Check for backfill every 10s
        max_depth: None,               // No limit on how far back to go
        desired_lag_slots: Some(5000), // Start backfilling if we are more than 5000 slots behind
    };

    let mut system_backfill_config = jupiter_backfill_config.clone();
    system_backfill_config.concurrency = 20; // System program has more transactions

    // 3. Configure Jupiter Indexer
    let jup_builder = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url.clone())
        .with_database(db_url.clone())
        .program_id(JUPITER_PROGRAM_ID)
        .with_poll_interval(30) // Poll every 30 seconds for Jupiter
        .with_batch_size(100)
        .with_backfill(jupiter_backfill_config);

    let config_jup = jup_builder.build()?;

    // 4. Configure System Indexer
    let sys_builder = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url.clone())
        .program_id(SYSTEM_PROGRAM_ID)
        .with_poll_interval(15) // Poll every 15 seconds for System Program
        .with_batch_size(100)
        .with_backfill(system_backfill_config);

    let config_sys = sys_builder.build()?;

    // 5. Create Indexers
    let mut indexer_jup = SolanaIndexer::new_with_storage(config_jup, storage.clone());
    let mut indexer_sys = SolanaIndexer::new_with_storage(config_sys, storage.clone());

    // 6. Initialize Schemas
    let db_pool = sqlx::PgPool::connect(&db_url).await?;
    let jup_handler = JupiterSwapHandler;
    let sys_handler = SystemTransferHandler;

    jup_handler.initialize_schema(&db_pool).await?;
    sys_handler.initialize_schema(&db_pool).await?;

    // 7. Register Decoders & Handlers
    indexer_jup.decoder_registry_mut()?.register(
        JUPITER_PROGRAM_ID.to_string(),
        Box::new(
            Box::new(JupiterInstructionDecoder) as Box<dyn InstructionDecoder<JupiterSwapEvent>>
        ),
    )?;
    indexer_jup.handler_registry_mut()?.register(
        JupiterSwapEvent::discriminator(),
        Box::new(Box::new(jup_handler) as Box<dyn EventHandler<JupiterSwapEvent>>),
    )?;

    indexer_sys.decoder_registry_mut()?.register(
        SYSTEM_PROGRAM_ID.to_string(),
        Box::new(
            Box::new(SystemTransferDecoder) as Box<dyn InstructionDecoder<SystemTransferEvent>>
        ),
    )?;
    indexer_sys.handler_registry_mut()?.register(
        SystemTransferEvent::discriminator(),
        Box::new(Box::new(sys_handler) as Box<dyn EventHandler<SystemTransferEvent>>),
    )?;

    // 8. Run Indexers
    println!("Running indexers. Live data will be processed and backfill will run if needed. Press Ctrl+C to stop.");

    let handle_jup = tokio::spawn(async move {
        println!("✅ [Jupiter] Indexer started.");
        if let Err(e) = indexer_jup.start().await {
            eprintln!("[Jupiter] Indexer failed: {}", e);
        }
    });

    let handle_sys = tokio::spawn(async move {
        println!("✅ [System] Indexer started.");
        if let Err(e) = indexer_sys.start().await {
            eprintln!("[System] Indexer failed: {}", e);
        }
    });

    // Wait for indexer tasks to complete. They will stop on Ctrl+C.
    let (jup_res, sys_res) = tokio::join!(handle_jup, handle_sys);
    if let Err(e) = jup_res {
        eprintln!("[Jupiter] Task join error: {}", e);
    }
    if let Err(e) = sys_res {
        eprintln!("[System] Task join error: {}", e);
    }

    println!("All indexers stopped.");

    Ok(())
}
