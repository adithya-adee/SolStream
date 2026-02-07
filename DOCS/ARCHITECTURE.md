# SolStream Architecture (Localnet Demo v0.1)

**Build Philosophy:** Start simple. Prove value on localnet before production complexity.

## 1. System Architecture

SolStream uses a **Simple Pipeline Model** optimized for localnet development and grant demonstration.

### Localnet Constraints:
- Single RPC endpoint (http://127.0.0.1:8899)
- No rate limiting needed
- Sequential processing acceptable
- Focus: Reliability over throughput

### The Pipeline Flow (Localnet Only)

1.  **Poller:** Calls `getSignaturesForAddress` every 2 seconds for target Program ID.
2.  **Fetcher:** For each new signature, calls `getTransaction` to retrieve full data.
3.  **Decoder:** Uses compiled IDL structs to parse instruction data into typed Rust objects.
4.  **Handler:** Executes user-defined logic (database insert, API call, etc).
5.  **Tracker:** Records processed signatures in `_solstream_processed` table to prevent re-processing.

### Simplified Flow Diagram

```
┌─────────────────┐
│ Localnet RPC    │
│ :8899           │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────┐
│  SolStream Core Loop (Single Thread)│
│                                     │
│  1. Poll signatures (2s interval)   │
│  2. Fetch transaction data          │
│  3. Decode via IDL structs          │
│  4. Check idempotency table         │
│  5. Call user handler               │
│  6. Mark signature as processed     │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────┐
│ Postgres/       │
│ Supabase        │
│                 │
│ • User tables   │
│ • _solstream_   │
│   processed     │
└─────────────────┘
```

## 2. Directory Structure

```
solstream-sdk/
├── Cargo.toml
├── idl/
│   └── your_program.json          // Developer drops IDL here
├── src/
│   ├── lib.rs                     // Public API
│   ├── poller.rs                  // RPC polling logic
│   ├── decoder.rs                 // Borsh + IDL parsing
│   ├── traits.rs                  // EventHandler trait
│   ├── storage.rs                 // Supabase/SQLx integration
│   └── macros/                    // Proc-macro for IDL compilation
└── examples/
    └── token_transfer_indexer.rs  // Demo implementation
```

### Build Flow:
1.  Developer adds `idl/program.json`
2.  Runs `cargo build` → Proc-macro generates structs
3.  Imports: `use solstream::generated::TransferEvent;`
4.  Implements `EventHandler<TransferEvent>` trait
5.  Runs indexer with `cargo run`

## 3. Error Handling

```rust
pub enum SolStreamError {
    DatabaseError(sqlx::Error),
    DecodingError(String),
    RpcError(String),
}
```

All errors propagate up and log to console. No automatic retries in v0.1 (localnet assumption: always available).

<h2> 4. Core Trait </h2>

```rust
#[async_trait]
pub trait EventHandler<T>: Send + Sync + 'static {
    async fn handle(&self, event: T, db: &PgPool, signature: &str)
        -> Result<(), SolStreamError>;
}
```

**Developer implements this once per event type. SDK calls it for each transaction.**

## 5. Developer Quickstart

### Step 1: Setup
```bash
# .env file
DATABASE_URL=postgresql://user:pass@localhost:5432/mydb
RPC_URL=http://127.0.0.1:8899
PROGRAM_ID=YourProgramPublicKey111111111111111111111
```

### Step 2: Implement Handler
```rust
use solstream::{EventHandler, SolStreamError};
use sqlx::PgPool;

pub struct TransferHandler;

#[async_trait]
impl EventHandler<TransferEvent> for TransferHandler {
    async fn handle(&self, event: TransferEvent, db: &PgPool, sig: &str)
        -> Result<(), SolStreamError> {
        sqlx::query!(
            "INSERT INTO transfers (signature, from_wallet, to_wallet, amount)
             VALUES ($1, $2, $3, $4)",
            sig, event.from.to_string(), event.to.to_string(), event.amount as i64
        )
        .execute(db)
        .await
        .map_err(SolStreamError::DatabaseError)?;
        Ok(())
    }
}
```

### Step 3: Run
```rust
#[tokio::main]
async fn main() -> Result<()> {
    SolStream::new()
        .with_rpc("http://127.0.0.1:8899")
        .with_database(env::var("DATABASE_URL")?)
        .program_id(env::var("PROGRAM_ID")?)
        .register_handler::<TransferEvent>(TransferHandler)
        .start()
        .await
}
```

**That's it. The indexer now polls localnet every 2s and processes transfers.**

## 6. Production Roadmap (Post-Grant)

-   **Phase 1 (Current):** Localnet polling, basic idempotency, SQLx support
-   **Phase 2:** WebSocket subscriptions, rate limiting, retry logic
-   **Phase 3:** Helius integration, gap-filling backfill, monitoring dashboard
-   **Phase 4:** Multi-program indexing, custom RPC providers, performance benchmarks vs existing solutions