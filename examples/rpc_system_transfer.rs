//! System Transfer Indexer Example
//!
//! This example demonstrates the Solana Indexer SDK's flexible architecture by building
//! a custom indexer for System Program transfers (native SOL transfers). It showcases:
//!
//! 1. **Custom Instruction Decoding** - Parsing System Program transfer instructions
//! 2. **Custom Event Handling** - Processing and storing transfer data in PostgreSQL
//! 3. **Multi-Program Support** - Using the DecoderRegistry pattern
//!
//! ## Architecture
//!
//! ```text
//! Transaction → InstructionDecoder<T> → Option<T> → EventHandler<T> → Database
//!               (Parse instruction)     (Typed      (Process event)
//!                                        Event)
//! ```
//!
//! ## Usage
//!
//! Set environment variables in `.env`:
//! ```env
//! RPC_URL=http://127.0.0.1:8899
//! DATABASE_URL=postgresql://postgres:password@localhost/solana_indexer_sdk
//! PROGRAM_ID=11111111111111111111111111111111  # System Program
//! ```
//!
//! Run the example:
//! ```bash
//! cargo run --example system_transfer_indexer
//! ```

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, EventDiscriminator, EventHandler, InstructionDecoder,
    SolanaIndexerConfigBuilder, SolanaIndexerError,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;

// ================================================================================================
// Event Definition
// Events are typed data structures that represent the specific blockchain action we're interested in.
// ================================================================================================

/// Represents a System Program transfer event (native SOL transfer).
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SystemTransferEvent {
    /// Source wallet public key
    pub from: Pubkey,
    /// Destination wallet public key
    pub to: Pubkey,
    /// Amount transferred in lamports
    pub amount: u64,
}

impl EventDiscriminator for SystemTransferEvent {
    /// Provides a unique identifier for the event type, allowing the SDK to route it to the correct handler.
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("SystemTransferEvent")
    }
}

// ================================================================================================
// Instruction Decoder
// Decoders are responsible for parsing raw Solana instructions and extracting typed events.
// ================================================================================================

/// Decoder for System Program transfer instructions.
pub struct SystemTransferDecoder;

impl InstructionDecoder<SystemTransferEvent> for SystemTransferDecoder {
    /// Decodes a Solana instruction into a System transfer event.
    fn decode(&self, instruction: &UiInstruction) -> Option<SystemTransferEvent> {
        match instruction {
            UiInstruction::Parsed(UiParsedInstruction::Parsed(parsed)) => {
                // Ensure the instruction belongs to the expected program
                if parsed.program != "system" {
                    return None;
                }

                let parsed_info = parsed.parsed.as_object()?;
                let instruction_type = parsed_info.get("type")?.as_str()?;

                // Only process 'transfer' instructions
                if instruction_type != "transfer" {
                    return None;
                }

                let info = parsed_info.get("info")?.as_object()?;

                // Extract and parse the transfer details
                let source = info.get("source")?.as_str()?;
                let destination = info.get("destination")?.as_str()?;
                let lamports = info.get("lamports")?.as_u64()?;

                Some(SystemTransferEvent {
                    from: source.parse().ok()?,
                    to: destination.parse().ok()?,
                    amount: lamports,
                })
            }
            _ => None,
        }
    }
}

// ================================================================================================
// Event Handler
// Handlers implement the custom business logic for processing each extracted event.
// ================================================================================================

/// Handler for System Program transfer events.
pub struct SystemTransferHandler;

#[async_trait]
impl EventHandler<SystemTransferEvent> for SystemTransferHandler {
    /// Initializes the database schema. This is called once on startup.
    async fn initialize_schema(&self, db: &PgPool) -> Result<(), SolanaIndexerError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS system_transfers (
                signature TEXT PRIMARY KEY,
                from_wallet TEXT NOT NULL,
                to_wallet TEXT NOT NULL,
                amount_lamports BIGINT NOT NULL,
                amount_sol DECIMAL(20, 9) GENERATED ALWAYS AS (amount_lamports / 1000000000.0) STORED,
                indexed_at TIMESTAMPTZ DEFAULT NOW()
            )",
        )
        .execute(db)
        .await?;

        Ok(())
    }

    /// Processes a decoded event. This is where you perform database operations,
    /// trigger webhooks, or update caches.
    async fn handle(
        &self,
        event: SystemTransferEvent,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;
        let sol_amount = event.amount as f64 / 1_000_000_000.0;

        println!(
            "📝 SOL Transfer: {} → {} ({:.9} SOL) [{}]",
            event.from, event.to, sol_amount, signature
        );

        // Store the event in the database using an idempotent query
        sqlx::query(
            "INSERT INTO system_transfers (signature, from_wallet, to_wallet, amount_lamports)
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

// ================================================================================================
// Main Application
// ================================================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    println!("🚀 System Transfer Indexer (Native SOL Transfers)\n");
    println!("This indexer monitors and stores all native SOL transfers on Solana.\n");

    // ============================================================================================
    // Configuration
    // ============================================================================================

    let rpc_url = std::env::var("RPC_URL")?;
    let database_url = std::env::var("DATABASE_URL")?;
    let program_id = std::env::var("PROGRAM_ID")
        .unwrap_or_else(|_| "11111111111111111111111111111111".to_string());

    println!("📋 Configuration:");
    println!("   RPC URL: {}", rpc_url);
    println!("   Database: {}", database_url);
    println!("   Program ID: {}", program_id);
    println!("   Program: System Program (Native SOL Transfers)\n");

    // Build indexer configuration
    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(database_url.clone())
        .program_id(program_id)
        .with_poll_interval(2) // Poll every 2 seconds for new transactions
        .with_batch_size(10) // Fetch up to 10 transactions per batch
        .build()?;

    // ============================================================================================
    // Indexer Setup
    // ============================================================================================

    // Create the indexer instance
    let mut indexer = solana_indexer_sdk::SolanaIndexer::new(config).await?;

    // Initialize database schema
    println!("📊 Initializing database schema...");
    let handler = SystemTransferHandler;
    let db_pool = sqlx::PgPool::connect(&database_url).await?;
    handler.initialize_schema(&db_pool).await?;
    println!("✅ Database schema ready\n");

    // ============================================================================================
    // Register Decoder
    // ============================================================================================

    println!("🔧 Registering instruction decoder...");

    // Register the System Transfer decoder with the decoder registry
    // The decoder registry matches by program name (e.g., "system", "spl-token")
    // NOT by program ID (the pubkey)
    indexer.decoder_registry_mut()?.register(
        "system".to_string(), // Program name as it appears in parsed instructions
        Box::new(
            Box::new(SystemTransferDecoder) as Box<dyn InstructionDecoder<SystemTransferEvent>>
        ),
    )?;

    println!("✅ Decoder registered for 'system' program\n");

    // ============================================================================================
    // Register Handler
    // ============================================================================================

    println!("🔧 Registering event handler...");

    // Wrap the handler in a Box for dynamic dispatch
    let handler_box: Box<dyn EventHandler<SystemTransferEvent>> = Box::new(handler);

    // Register the handler with the indexer
    // The SDK will automatically route SystemTransferEvent instances to this handler
    indexer
        .handler_registry_mut()?
        .register(SystemTransferEvent::discriminator(), Box::new(handler_box))?;

    println!("✅ Handler registered\n");

    // ============================================================================================
    // Start Indexing
    // ============================================================================================

    println!("🔄 Starting indexer...");
    println!("   Monitoring native SOL transfers on Solana");
    println!("   Compatible with spl-transfer-generator");
    println!("   Press Ctrl+C to stop\n");
    println!("{}", "=".repeat(80));

    // Start the indexer - this runs indefinitely until interrupted
    indexer.start().await?;

    Ok(())
}
