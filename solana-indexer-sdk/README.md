# Solana Indexer SDK

The `solana-indexer-sdk` is a lightweight, customizable, and high-performance SDK for indexing data from the Solana blockchain.

## Features

- **Multi-Program Indexing:** Index transactions from multiple programs simultaneously.
- **Customizable:** Easily implement custom logic for decoding and handling events.
- **High-Performance:** Built with performance in mind, using `tokio` for concurrency.
- **Backfill Support:** Index historical data with the backfill engine.
- **Reorg Handling:** Automatic detection and handling of chain reorganizations.

## Getting Started

To get started, add the `solana-indexer-sdk` to your `Cargo.toml`:

```toml
[dependencies]
solana-indexer-sdk = "0.1.0"
```

Then, see the [examples](/examples) for usage.
