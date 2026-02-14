use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexer,
    SolanaIndexerConfigBuilder, SolanaIndexerError, Storage,
};
// use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;
use std::sync::Arc;
// use std::time::Duration;

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
    async fn handle(
        &self,
        event: SystemTransferEvent,
        context: &solana_indexer_sdk::TxMetadata,
        _db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;
        println!(
            "[System] Transfer: {} -> {} ({} lamports) | Sig: {:.8}...",
            event.from, event.to, event.amount, signature
        );
        Ok(())
    }
}

// --------------------------------------------------------
// Program 2: Memo Program (Simple example)
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
        match instruction {
            UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) => {
                if parsed.program_id != MEMO_PROGRAM_ID {
                    return None;
                }
                // Memo usually just has the message as string
                // But typically it comes as ParticallyDecoded with raw data if not fully parsed by RPC
                // Or "parsed": string.
                // Actually Memo program is simple, often just raw bytes -> string.
                // Let's assume generic parsed or partially decoded.
                None // Skip complex parsed logic for example brevity, fall through to partially decoded
            }
            UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(decoded)) => {
                if decoded.program_id != MEMO_PROGRAM_ID {
                    return None;
                }
                // Decode data (Base58)
                let data_bytes = solana_sdk::bs58::decode(&decoded.data).into_vec().ok()?;
                let message =
                    String::from_utf8(data_bytes).unwrap_or_else(|_| "<binary>".to_string());
                Some(MemoEvent { message })
            }
            _ => None,
        }
    }
}

pub struct MemoHandler;

#[async_trait]
impl EventHandler<MemoEvent> for MemoHandler {
    async fn handle(
        &self,
        event: MemoEvent,
        context: &solana_indexer_sdk::TxMetadata,
        _db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;
        println!(
            "[Memo] Message: \"{}\" | Sig: {:.8}...",
            event.message, signature
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    println!("Starting Multi-Program Indexer (System + Memo)...");

    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/solana_indexer_sdk".to_string()
    });

    // 1. Create Shared Storage
    // By sharing the storage instance, multiple indexers can coordinate on processed
    // slots and signatures, ensuring atomic updates across different programs.
    println!("Initializing shared storage...");
    let storage = Arc::new(Storage::new(&db_url).await?);
    // Initialize the core SDK tables (processed_transactions, etc.)
    storage.initialize().await?;

    // 2. Configure Indexer 1 (System Program)
    // Each indexer can have its own polling interval and batch size tailored to
    // the program's transaction volume.
    let config_system = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url.clone())
        .with_database(db_url.clone())
        .program_id(SYSTEM_PROGRAM_ID)
        .with_poll_interval(10)
        .with_batch_size(5)
        .build()?;

    // 3. Configure Indexer 2 (Memo Program)
    let config_memo = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url)
        .program_id(MEMO_PROGRAM_ID)
        .with_poll_interval(15)
        .with_batch_size(5)
        .build()?;

    // 4. Create Indexers
    // Using `new_with_storage` allows us to inject the shared database connection pool.
    let mut indexer_system = SolanaIndexer::new_with_storage(config_system, storage.clone());
    let mut indexer_memo = SolanaIndexer::new_with_storage(config_memo, storage.clone());

    // 5. Register Decoders & Handlers

    // System
    indexer_system.decoder_registry_mut()?.register(
        "system".to_string(),
        Box::new(
            Box::new(SystemTransferDecoder) as Box<dyn InstructionDecoder<SystemTransferEvent>>
        ),
    )?;
    let system_handler: Box<dyn EventHandler<SystemTransferEvent>> =
        Box::new(SystemTransferHandler);
    indexer_system.handler_registry_mut()?.register(
        SystemTransferEvent::discriminator(),
        Box::new(system_handler),
    )?;

    // Memo
    indexer_memo.decoder_registry_mut()?.register(
        "memo".to_string(),
        Box::new(Box::new(MemoDecoder) as Box<dyn InstructionDecoder<MemoEvent>>),
    )?;
    let memo_handler: Box<dyn EventHandler<MemoEvent>> = Box::new(MemoHandler);
    indexer_memo
        .handler_registry_mut()?
        .register(MemoEvent::discriminator(), Box::new(memo_handler))?;

    // 6. Run Concurrent Indexers
    println!("Running indexers concurrently...");

    // tokio::select! or join?
    // We want them both to run.

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

    // Wait for Ctrl+C to shut down
    match tokio::signal::ctrl_c().await {
        Ok(()) => println!("Shutting down..."),
        Err(e) => eprintln!("Error listening for shutdown: {}", e),
    }

    // Creating indexers consumes them, so we can't call shutdown() on them here unless we kept handles/tokens.
    // But `start()` listens for Ctrl+C internally!
    // Both indexers will see the Ctrl+C signal and shutdown independently.
    // So we just await their handles.

    let _ = tokio::join!(handle_system, handle_memo);

    println!("All indexers stopped.");
    Ok(())
}
