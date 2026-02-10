use honggfuzz::fuzz;
use solana_indexer::{Storage, utils::logging::log_section};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    dotenvy::dotenv().ok();

    log_section("Starting Storage Fuzzing");

    // Skip if DATABASE_URL not set
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("DATABASE_URL not set, skipping storage benchmarks");
            return;
        }
    };

    let storage = rt.block_on(async {
        let s = Storage::new(&database_url)
            .await
            .expect("Failed to connect");
        s.initialize().await.expect("Failed to initialize");
        std::sync::Arc::new(s)
    });

    let counter = AtomicU64::new(0);

    loop {
        fuzz!(|data: &[u8]| {
            let storage = storage.clone();
            rt.block_on(async {
                if data.is_empty() {
                    return;
                }

                // Use the first byte to determine the operation
                match data[0] % 3 {
                    0 => {
                        let _ = storage.is_processed("bench_signature_123").await;
                    }
                    1 => {
                        let val = counter.fetch_add(1, Ordering::Relaxed);
                        let sig = format!(
                            "bench_sig_{}_{}",
                            val,
                            data[1..]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<String>()
                        );
                        let _ = storage.mark_processed(&sig, 12345).await;
                    }
                    2 => {
                        let _ = storage.get_last_processed_slot().await;
                    }
                    _ => {}
                }
            });
        });
    }
}
