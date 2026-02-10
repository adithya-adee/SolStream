use honggfuzz::fuzz;
use serde_json::json;
use solana_indexer::utils::logging::log_section;
use solana_indexer::{SolanaIndexer, SolanaIndexerConfigBuilder, Storage};
use std::sync::Arc;
use tokio::runtime::Runtime;
use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn main() {
    // SAFETY: This is a benchmark setup, no other threads should be accessing env vars concurrently.
    unsafe { std::env::set_var("SOLANA_INDEXER_SILENT", "1") };
    let rt = Runtime::new().unwrap();
    dotenvy::dotenv().ok();

    log_section("Starting Pipeline Fuzzing/Benchmark");

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("DATABASE_URL not set, skipping fuzzing");
            return;
        }
    };

    loop {
        fuzz!(|data: &[u8]| {
            rt.block_on(async {
                // Determine batch size from data
                let batch_size = if data.is_empty() { 1 } else { (data[0] as usize % 50) + 1 };
                let mock_server = MockServer::start().await;

                // Setup mocks
                Mock::given(method("POST"))
                    .and(body_string_contains("getVersion"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "result": { "solana-core": "1.16.7", "feature-set": 0 },
                        "id": 1
                    })))
                    .mount(&mock_server)
                    .await;

                Mock::given(method("POST"))
                    .and(body_string_contains("getLatestBlockhash"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "context": { "slot": 1 },
                            "value": { "blockhash": "hash", "lastValidBlockHeight": 100 }
                        },
                        "id": 1
                    })))
                    .mount(&mock_server)
                    .await;

                // Mock getSignaturesForAddress
                let mut signatures = Vec::new();
                for i in 0..batch_size {
                    // Use fuzz data to vary signature if possible
                    let suffix = if data.len() > i + 1 { format!("{:02x}", data[i+1]) } else { format!("{}", i) };
                    signatures.push(json!({
                        "signature": format!("fuzz_sig_{}", suffix),
                        "slot": 100 + i,
                        "err": null,
                        "memo": null,
                        "blockTime": 1000
                    }));
                }

                Mock::given(method("POST"))
                    .and(body_string_contains("getSignaturesForAddress"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "result": signatures,
                        "id": 1
                    })))
                    .mount(&mock_server)
                    .await;

                // Mock getTransaction batch
                Mock::given(method("POST"))
                    .and(body_string_contains("getTransaction"))
                    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "slot": 100,
                            "transaction": {
                                "signatures": ["fuzz_sig_0"], // Simplified for mock
                                "message": { "accountKeys": [], "instructions": [], "recentBlockhash": "hash" }
                            },
                            "meta": { "err": null, "status": { "Ok": null }, "fee": 0, "preBalances": [], "postBalances": [], "innerInstructions": [] }
                        },
                        "id": 1
                    })))
                    .mount(&mock_server)
                    .await;

                let storage = Arc::new(Storage::new(&database_url).await.expect("DB"));
                storage.initialize().await.expect("Init");

                let config = SolanaIndexerConfigBuilder::new()
                    .with_rpc(mock_server.uri())
                    .with_database(&database_url)
                    .program_id("11111111111111111111111111111111")
                    .with_batch_size(batch_size)
                    .with_poll_interval(1)
                    .build()
                    .unwrap();

                let indexer = SolanaIndexer::new_with_storage(config, storage);

                // Run for short duration to exercise the pipeline
                let _ = tokio::time::timeout(std::time::Duration::from_millis(50), indexer.start()).await;
            });
        });
    }
}
