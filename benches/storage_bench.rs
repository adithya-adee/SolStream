use solana_indexer::{Storage, StorageBackend};
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;

fn main() {
    let rt = Runtime::new().unwrap();
    // dotenvy::dotenv().ok(); // Optional

    // Skip if DATABASE_URL not set
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("DATABASE_URL not set, skipping storage benchmarks");
            return;
        }
    };

    println!("Starting Storage Benchmark...");

    // Initialize storage
    let storage = rt.block_on(async {
        let s = Storage::new(&database_url)
            .await
            .expect("Failed to connect");
        s.initialize().await.expect("Failed to initialize");
        Arc::new(s)
    });

    let iterations = 10_000; // Database ops are slower, use fewer iterations
    let start = Instant::now();

    rt.block_on(async {
        let backend: Arc<dyn StorageBackend> = storage.clone();

        for i in 0..iterations {
            let sig = format!("bench_simple_sig_{}", i);

            // Write
            let _ = backend.mark_processed(&sig, 12345).await;

            // Read
            if i % 10 == 0 {
                let _ = backend.is_processed(&sig).await;
            }

            if i % 1000 == 0 {
                print!(".");
                use std::io::Write;
                std::io::stdout().flush().unwrap();
            }
        }
    });

    let duration = start.elapsed();
    println!("\nPerformed {} storage ops in {:?}", iterations, duration);
    println!(
        "Throughput: {:.2} ops/s",
        iterations as f64 / duration.as_secs_f64()
    );
}
