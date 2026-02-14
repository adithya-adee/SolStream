use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    calculate_discriminator, EventDiscriminator, EventHandler, InstructionDecoder, SolanaIndexer,
    SolanaIndexerConfigBuilder, SolanaIndexerError,
};
// use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::{UiInstruction, UiParsedInstruction};
use sqlx::PgPool;
// use std::str::FromStr;

// Raydium AMM v4 Program ID
const RAYDIUM_V4_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

// Define the event we want to produce
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct RaydiumSwapEvent {
    pub amount_in: u64,
    pub min_amount_out: u64,
    pub user: String, // We'll try to extract the user (signer)
}

impl EventDiscriminator for RaydiumSwapEvent {
    fn discriminator() -> [u8; 8] {
        calculate_discriminator("RaydiumSwapEvent")
    }
}

// Implement the decoder
pub struct RaydiumSwapDecoder;

impl InstructionDecoder<RaydiumSwapEvent> for RaydiumSwapDecoder {
    fn decode(&self, instruction: &UiInstruction) -> Option<RaydiumSwapEvent> {
        match instruction {
            UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(decoded)) => {
                // Check if it's the Raydium program
                if decoded.program_id != RAYDIUM_V4_PROGRAM_ID {
                    return None;
                }

                // Decode data (Base58)
                let data_bytes = solana_sdk::bs58::decode(&decoded.data).into_vec().ok()?;

                // Raydium SwapBaseIn Instruction Index is 3
                // Layout: [u8; 1] (instruction), [u64; 1] (amount_in), [u64; 1] (min_amount_out)
                if data_bytes.len() < 17 || data_bytes[0] != 3 {
                    return None;
                }

                // Parse amounts (little endian)
                let amount_in = u64::from_le_bytes(data_bytes[1..9].try_into().ok()?);
                let min_amount_out = u64::from_le_bytes(data_bytes[9..17].try_into().ok()?);

                // User is usually the first signer or the token source owner.
                // In Raydium Swap V4 accounts, the user (TokenAuthority) is often account 17 or similar depending on the exact path.
                // But simplified, let's just grab the first account as a placeholder if we can't be sure without parsing all accounts.
                // actually, `decoded.accounts` is a Vec<String> (Pubkeys).
                let user = decoded
                    .accounts
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                Some(RaydiumSwapEvent {
                    amount_in,
                    min_amount_out,
                    user,
                })
            }
            _ => None,
        }
    }
}

// Implement the handler
pub struct RaydiumSwapHandler;

#[async_trait]
impl EventHandler<RaydiumSwapEvent> for RaydiumSwapHandler {
    async fn handle(
        &self,
        event: RaydiumSwapEvent,
        context: &solana_indexer_sdk::TxMetadata,
        _db: &PgPool,
    ) -> Result<(), SolanaIndexerError> {
        let signature = &context.signature;
        println!("🦄 Raydium Swap Detected! Sig: {:.8}...", signature);
        println!(
            "   In: {} | Min Out: {} | User: {}",
            event.amount_in, event.min_amount_out, event.user
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    // Basic setup
    println!("Starting Raydium Indexer Example...");

    // Check for specific RPC URL or use a public one (likely to rate limit or fail for Raydium volume)
    let rpc_url = std::env::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    // Use a mock DB URL if not provided, just to let it start (indexer will fail if it tries to connect but maybe we can mock it? No, need separate example or docker)
    // For this example to actually runs logic, it needs a DB.
    // We assume the user has a DB or will read the error.
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://postgres:password@localhost:5432/solana_indexer_sdk".to_string()
    });

    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url)
        .program_id(RAYDIUM_V4_PROGRAM_ID)
        .with_poll_interval(10) // Poll less frequently for public RPC
        .with_batch_size(5) // Reduce request concurrency to avoid rate limits
        .build()?;

    let mut indexer = SolanaIndexer::new(config).await?;

    // Register Decoder
    indexer.decoder_registry_mut()?.register(
        RAYDIUM_V4_PROGRAM_ID.to_string(),
        Box::new(Box::new(RaydiumSwapDecoder) as Box<dyn InstructionDecoder<RaydiumSwapEvent>>),
    )?;

    // Register Handler
    let handler: Box<dyn EventHandler<RaydiumSwapEvent>> = Box::new(RaydiumSwapHandler);
    indexer
        .handler_registry_mut()?
        .register(RaydiumSwapEvent::discriminator(), Box::new(handler))?;

    // Start with graceful shutdown support
    let indexer_handle = tokio::spawn(async move {
        if let Err(e) = indexer.start().await {
            eprintln!("Indexer error: {}", e);
        }
    });

    // Wait for Ctrl+C
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            println!("Shutting down...");
        }
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // In a real app we'd use indexer.shutdown() via a shared handle or channel,
    // but SolanaIndexer::start captures self.
    // The previous implementation added a shutdown() method but start() consumes self.
    // However, the start() method inside spawns a ctrl-c handler itself!
    // So we actually just need to await the handle or let it run.

    // Since start() spawns its own ctrl-c listener (as per my previous edit),
    // we actually don't need this check here if we want to rely on that.
    // BUT main() returning will kill the spawned task.
    // So we should just await the indexer handle.

    let _ = indexer_handle.await;

    Ok(())
}
