//! An example demonstrating how to run multiple, independent indexers within a single process.
//!
//! This is useful for indexing different programs that do not need to share transaction
//! data but can share a database connection. Each indexer runs in its own concurrent task.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, config::BackfillConfig, EventDiscriminator, EventHandler,
    InstructionDecoder, SolanaIndexer, SolanaIndexerConfigBuilder, SolanaIndexerError,
};
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

// --------------------------------------------------------
// Program 1: System Program (Transfer)
// --------------------------------------------------------
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

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
        if let UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) = instruction {
            if parsed.program_id == SYSTEM_PROGRAM_ID
                && parsed.parsed.get("type")?.as_str()? == "transfer"
            {
                let info = parsed.parsed.get("info")?;
                return Some(SystemTransferEvent {
                    from: info.get("source")?.as_str()?.to_string(),
                    to: info.get("destination")?.as_str()?.to_string(),
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
            "CREATE TABLE IF NOT EXISTS system_transfers_multi (
                signature TEXT PRIMARY KEY,
                from_address TEXT,
                to_address TEXT,
                amount BIGINT
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
        sqlx::query(
            "INSERT INTO system_transfers_multi (signature, from_address, to_address, amount)
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
        )
        .bind(&context.signature)
        .bind(&event.from)
        .bind(&event.to)
        .bind(event.amount as i64)
        .execute(db)
        .await?;
        Ok(())
    }
}

// --------------------------------------------------------
// Program 2: Memo Program
// --------------------------------------------------------
const MEMO_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcQb";

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct MemoEvent {
    pub message: String,
}

impl EventDiscriminator for MemoEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("MemoEvent")
    }
}

pub struct MemoDecoder;
impl InstructionDecoder<MemoEvent> for MemoDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<MemoEvent> {
        if let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(decoded)) = instruction {
            if decoded.program_id == MEMO_PROGRAM_ID {
                let data_bytes = solana_sdk::bs58::decode(&decoded.data).into_vec().ok()?;
                let message = String::from_utf8(data_bytes).ok()?;
                return Some(MemoEvent { message });
            }
        }
        None
    }
}

pub struct MemoHandler;
#[async_trait]
impl EventHandler<MemoEvent> for MemoHandler {
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS memos_multi (
                signature TEXT,
                message TEXT,
                PRIMARY KEY (signature, message)
            )",
        )
        .execute(db)
        .await?;
        Ok(())
    }
    async fn handle(
        &self,
        event: MemoEvent,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "INSERT INTO memos_multi (signature, message)
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&context.signature)
        .bind(&event.message)
        .execute(db)
        .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("🚀 Starting Multi-Program Indexer (System + Memo)...");

    let rpc_url = "https://api.mainnet-beta.solana.com".to_string();
    let db_url = std::env::var("DATABASE_URL")?;

    let backfill_config = BackfillConfig {
        enabled: true,
        ..Default::default()
    };

    // 1. Configure Indexer 1 (System Program)
    let config_system = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url.clone())
        .with_database(db_url.clone())
        .program_id(SYSTEM_PROGRAM_ID)
        .with_backfill(backfill_config.clone())
        .build()?;

    // 2. Configure Indexer 2 (Memo Program)
    let config_memo = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url.clone())
        .program_id(MEMO_PROGRAM_ID)
        .with_backfill(backfill_config)
        .build()?;

    // 3. Create and register components for both indexers.
    let mut indexer_system = SolanaIndexer::new(config_system).await?;
    let system_handler = SystemTransferHandler;
    system_handler
        .initialize_schema(&sqlx::PgPool::connect(&db_url).await?)
        .await?;
    indexer_system.register_decoder("system", SystemTransferDecoder)?;
    indexer_system.register_handler(system_handler)?;

    let mut indexer_memo = SolanaIndexer::new(config_memo).await?;
    let memo_handler = MemoHandler;
    memo_handler
        .initialize_schema(&sqlx::PgPool::connect(&db_url).await?)
        .await?;
    indexer_memo.register_decoder(MEMO_PROGRAM_ID, MemoDecoder)?;
    indexer_memo.register_handler(memo_handler)?;

    // 4. Run indexers concurrently.
    println!("Running indexers concurrently. Press Ctrl+C to stop.");

    let handle_system = tokio::spawn(async move {
        if let Err(e) = indexer_system.start().await {
            eprintln!("System Indexer failed: {}", e);
        }
    });

    let handle_memo = tokio::spawn(async move {
        if let Err(e) = indexer_memo.start().await {
            eprintln!("Memo Indexer failed: {}", e);
        }
    });

    // The internal `start` method handles graceful shutdown on Ctrl+C.
    // We just await the handles to keep the main process alive.
    let _ = tokio::join!(handle_system, handle_memo);

    println!("All indexers stopped.");
    Ok(())
}
