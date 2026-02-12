use serde_json::json;
use solana_indexer::{SolanaIndexer, SolanaIndexerConfigBuilder, Storage, StorageBackend};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

// Simple mock RPC server using raw TCP/HTTP to avoid heavy dependencies like wiremock
async fn start_mock_rpc_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(conn) => conn,
                Err(_) => continue,
            };

            tokio::spawn(async move {
                let mut buf = [0; 4096];
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };

                let request = String::from_utf8_lossy(&buf[..n]);

                // Very basic request parsing
                let response_body = if request.contains("getVersion") {
                    json!({
                        "jsonrpc": "2.0",
                        "result": { "solana-core": "1.16.7", "feature-set": 0 },
                        "id": 1
                    })
                } else if request.contains("getLatestBlockhash") {
                    json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "context": { "slot": 1 },
                            "value": { "blockhash": "hash", "lastValidBlockHeight": 100 }
                        },
                        "id": 1
                    })
                } else if request.contains("getSignaturesForAddress") {
                    let signatures: Vec<_> = (0..50)
                        .map(|i| {
                            json!({
                                "signature": format!("bench_sig_{}", i),
                                "slot": 100 + i,
                                "err": null,
                                "memo": null,
                                "blockTime": 1000
                            })
                        })
                        .collect();

                    json!({
                        "jsonrpc": "2.0",
                        "result": signatures,
                        "id": 1
                    })
                } else if request.contains("getTransaction") {
                    json!({
                        "jsonrpc": "2.0",
                        "result": {
                            "slot": 100,
                            "transaction": {
                                "signatures": ["bench_sig_0"],
                                "message": { "accountKeys": [], "instructions": [], "recentBlockhash": "hash" }
                            },
                            "meta": { "err": null, "status": { "Ok": null }, "fee": 0, "preBalances": [], "postBalances": [], "innerInstructions": [] }
                        },
                        "id": 1
                    })
                } else {
                    json!({ "jsonrpc": "2.0", "error": { "code": -32600, "message": "Invalid Request" }, "id": 1 })
                };

                let response_json = response_body.to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    response_json.len(),
                    response_json
                );

                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });

    format!("http://{}", addr)
}

fn main() {
    // SAFETY: This is a benchmark setup
    unsafe { std::env::set_var("SOLANA_INDEXER_SILENT", "1") };

    let rt = Runtime::new().unwrap();

    println!("Starting Pipeline Latency Benchmark...");

    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("DATABASE_URL not set, skipping fuzzing");
            return;
        }
    };

    let (storage, rpc_url) = rt.block_on(async {
        let storage = Arc::new(
            Storage::new(&database_url)
                .await
                .expect("DB connection failed"),
        );
        storage
            .initialize()
            .await
            .expect("DB initialization failed");

        let rpc_url = start_mock_rpc_server().await;

        (storage, rpc_url)
    });

    let storage_backend: Arc<dyn StorageBackend> = storage;

    // Run a single throughput test
    rt.block_on(async {
        let config = SolanaIndexerConfigBuilder::new()
            .with_rpc(&rpc_url)
            .with_database(&database_url)
            .program_id("11111111111111111111111111111111")
            .with_batch_size(50)
            .with_poll_interval(1)
            .build()
            .unwrap();

        let indexer = SolanaIndexer::new_with_storage(config, storage_backend.clone());

        let start = Instant::now();

        // Run for 5 seconds
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), indexer.start()).await;

        let duration = start.elapsed();
        println!("Ran pipeline for {:?}", duration);
    });
}
