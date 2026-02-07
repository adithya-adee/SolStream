# Product Requirement Document: SolStream SDK

## 1. Executive Summary

SolStream is an open-source, high-performance Solana indexing SDK written in Rust. It allows developers to build custom indexers by simply providing an Anchor IDL and writing their business logic. The SDK handles the heavy lifting: RPC polling, transaction fetching, and IDL-based decoding.

**Current Scope:** Localnet-optimized proof of concept. Production features (WebSocket, Helius, Mainnet gap-filling) planned post-grant.

## 2. Problem Statement

-   **Infrastructure Complexity:** Developers spend days building reliable RPC polling loops and handling retries.
-   **Decoding Overhead:** Manually decoding Borsh-serialized transaction data from hex blobs is error-prone.
-   **Repetitive Boilerplate:** Every indexer rewrites the same transaction fetching and signature tracking logic.

## 3. Goals & Objectives (Localnet Demo)

-   **Developer First:** Reduce time to "First Indexed Event" from days to under 30 minutes.
-   **Type Safety:** Leverage Rust + Anchor IDL for compile-time guarantees.
-   **Reliability:** Basic idempotency to prevent duplicate processing during restarts.
-   **Post-Grant Roadmap:** WebSocket support, Helius integration, Mainnet gap-filling, production monitoring.

## 4. Key Features (v0.1 - Localnet)

-   **IDL-to-Struct Mapping:** Automatically decode Anchor events/instructions into Rust structs.
-   **RPC Polling Engine:** Simple, reliable transaction fetching from localnet RPC.
-   **Database Agnostic:** Native SQLx support (Postgres/Supabase) with extensible traits.
-   **Basic Idempotency:** Signature tracking to prevent duplicate inserts on restart.
-   **Coming Soon:** WebSocket streams, rate limiting, gap-filling, Helius provider integration.

## 5. Target Audience

-   Solana dapp developers needing custom backends.
-   Analytics platforms requiring real-time program monitoring.
-   DAO tooling requiring event-driven automation.

## 6. Grant Application Pitch

-   **Problem Size:** Every Solana dapp needs an indexer. Current options: expensive SaaS (Helius) or weeks of custom dev.
-   **Solution Uniqueness:** First Rust SDK to combine IDL auto-decoding + plug-and-play handlers.
-   **Ecosystem Impact:** Lowers barrier for 1000+ developers to build production backends.

### Demo Deliverables:

-   Working localnet indexer for token transfers
-   Full documentation + quickstart guide
-   Benchmark: <100 lines of user code for full indexer

### Post-Grant Vision:

Mainnet production SDK competing with Helius for developer mindshare.