//! Example of an indexer tracking Solana accounts state.
//!
//! This example shows how to configure the indexer to decode account data
//! using the enhanced `AccountDecoder` trait that provides the account's Pubkey.

use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_indexer_sdk::{
    types::traits::EventHandler, AccountDecoder, EventDiscriminator, Result, SchemaInitializer,
    SolanaIndexer, SolanaIndexerConfigBuilder,
};
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use sqlx::PgPool;

// 1. Define your Account data structure.
// It now includes the `pubkey` of the account.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct UserProfile {
    pub pubkey: Pubkey,
    pub discriminator: [u8; 8],
    pub username: String,
    pub reputation: u64,
}

// 2. Implement EventDiscriminator for event routing.
impl EventDiscriminator for UserProfile {
    fn discriminator() -> [u8; 8] {
        // This should match the first 8 bytes of your on-chain account data (e.g., from Anchor).
        [101, 202, 103, 204, 105, 206, 107, 208]
    }
}

// 3. Implement the enhanced AccountDecoder.
pub struct UserProfileDecoder;

impl AccountDecoder<UserProfile> for UserProfileDecoder {
    // The decode method now receives the account's pubkey.
    fn decode(&self, pubkey: &Pubkey, account: &Account) -> Option<UserProfile> {
        if account.data.len() < 8 || account.data[0..8] != UserProfile::discriminator() {
            return None;
        }

        // Deserialize the account data.
        let mut data = UserProfile::try_from_slice(&account.data).ok()?;
        // Add the pubkey to our decoded struct.
        data.pubkey = *pubkey;
        Some(data)
    }
}

// 4. Implement the EventHandler to process the decoded account data.
pub struct UserProfileHandler;

#[async_trait]
impl EventHandler<UserProfile> for UserProfileHandler {
    async fn handle(
        &self,
        event: UserProfile,
        context: &solana_indexer_sdk::TxMetadata,
        db: &PgPool,
    ) -> Result<()> {
        println!(
            "✅ Found UserProfile update for {} in tx {}: {} (Rep: {})",
            event.pubkey,
            &context.signature[..8],
            event.username,
            event.reputation
        );

        // Use the pubkey as the primary key for a robust upsert.
        sqlx::query(
            "INSERT INTO users (pubkey, username, reputation, last_signature)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (pubkey) DO UPDATE SET
                username = EXCLUDED.username,
                reputation = EXCLUDED.reputation,
                last_signature = EXCLUDED.last_signature",
        )
        .bind(event.pubkey.to_string())
        .bind(&event.username)
        .bind(event.reputation as i64)
        .bind(&context.signature)
        .execute(db)
        .await?;

        Ok(())
    }
}

// 5. Implement SchemaInitializer to create the database table.
pub struct UserSchemaInitializer;

#[async_trait]
impl SchemaInitializer for UserSchemaInitializer {
    async fn initialize(&self, db: &PgPool) -> Result<()> {
        println!("🛠️  Initializing User Schema...");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                pubkey TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                reputation BIGINT,
                last_signature TEXT
            )",
        )
        .execute(db)
        .await?;
        println!("✅ User Schema Initialized");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let rpc_url = std::env::var("RPC_URL").unwrap_or_else(|_| "http://127.0.0.1:8899".to_string());
    let db_url = std::env::var("DATABASE_URL")?;
    let program_id_str = std::env::var("PROGRAM_ID")?;

    println!("🚀 Starting Account Indexer (Live Mode)");

    // Configure the indexer. Note we do not need to set the indexing mode manually.
    let config = SolanaIndexerConfigBuilder::new()
        .with_rpc(rpc_url)
        .with_database(db_url.clone())
        .program_id(program_id_str)
        .build()?;

    let mut indexer = SolanaIndexer::new(config).await?;

    // Register all components.
    indexer.register_schema_initializer(Box::new(UserSchemaInitializer));

    // Registering the account decoder automatically enables `accounts` mode.
    indexer.register_account_decoder(UserProfileDecoder)?;

    // Register the handler for our decoded `UserProfile` type.
    indexer.register_handler(UserProfileHandler)?;

    println!("✅ Registered Decoder, Handler, and Schema");
    println!("🔄 Starting Indexer Loop... Press Ctrl+C to stop gracefully.\n");

    let indexer_handle = tokio::spawn(async move { indexer.start().await });

    // Wait for shutdown signal.
    if let Err(e) = tokio::signal::ctrl_c().await {
        eprintln!("Error waiting for shutdown signal: {}", e);
    }

    // Wait for the indexer to finish.
    if let Err(e) = indexer_handle.await {
        eprintln!("Indexer task failed: {:?}", e);
    }

    println!("✅ Indexer shut down.");
    Ok(())
}
