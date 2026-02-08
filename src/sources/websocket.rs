//! WebSocket-based transaction source for real-time indexing
//!
//! This module provides a WebSocket client that subscribes to Solana program
//! notifications and yields transaction signatures in real-time.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use std::str::FromStr;
use tokio::time::{Duration, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::TransactionSource;
use crate::common::error::{Result, SolanaIndexerError};

/// WebSocket transaction source
///
/// Connects to Solana's WebSocket RPC and subscribes to program notifications.
/// Automatically handles reconnection on disconnect.
pub struct WebSocketSource {
    /// WebSocket URL (ws:// or wss://)
    ws_url: String,
    /// Program ID to subscribe to
    program_id: Pubkey,
    /// Reconnection delay in seconds
    reconnect_delay_secs: u64,
    /// Internal state
    state: WebSocketState,
}

/// Internal WebSocket state
enum WebSocketState {
    Disconnected,
    Connected {
        #[allow(dead_code)] // Kept for future unsubscribe functionality
        subscription_id: u64,
        receiver: tokio::sync::mpsc::UnboundedReceiver<Signature>,
    },
}

/// WebSocket notification from Solana
#[derive(Debug, Deserialize)]
struct ProgramNotification {
    params: NotificationParams,
}

#[derive(Debug, Deserialize)]
struct NotificationParams {
    result: NotificationResult,
}

#[derive(Debug, Deserialize)]
struct NotificationResult {
    value: NotificationValue,
}

#[derive(Debug, Deserialize)]
struct NotificationValue {
    signature: String,
}

/// Subscription response from Solana
#[derive(Debug, Deserialize)]
struct SubscriptionResponse {
    result: u64,
}

impl WebSocketSource {
    /// Creates a new WebSocket source
    ///
    /// # Arguments
    ///
    /// * `ws_url` - WebSocket URL (e.g., "ws://127.0.0.1:8900")
    /// * `program_id` - Program ID to subscribe to
    /// * `reconnect_delay_secs` - Delay between reconnection attempts
    pub fn new(ws_url: impl Into<String>, program_id: Pubkey, reconnect_delay_secs: u64) -> Self {
        Self {
            ws_url: ws_url.into(),
            program_id,
            reconnect_delay_secs,
            state: WebSocketState::Disconnected,
        }
    }

    /// Connects to WebSocket and subscribes to program notifications
    async fn connect(&mut self) -> Result<()> {
        use crate::common::logging;

        logging::log(
            logging::LogLevel::Info,
            &format!("Connecting to WebSocket: {}", self.ws_url),
        );

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.ws_url).await.map_err(|e| {
            SolanaIndexerError::RpcError(format!("WebSocket connection failed: {e}"))
        })?;

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to program
        let subscribe_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "programSubscribe",
            "params": [
                self.program_id.to_string(),
                {
                    "encoding": "jsonParsed",
                    "commitment": "confirmed"
                }
            ]
        });

        write
            .send(Message::Text(subscribe_request.to_string()))
            .await
            .map_err(|e| {
                SolanaIndexerError::RpcError(format!("Failed to send subscription: {e}"))
            })?;

        // Wait for subscription confirmation
        let subscription_id = loop {
            #[allow(clippy::collapsible_if)]
            if let Some(Ok(Message::Text(text))) = read.next().await {
                if let Ok(response) = serde_json::from_str::<SubscriptionResponse>(&text) {
                    break response.result;
                }
            }
        };

        logging::log(
            logging::LogLevel::Success,
            &format!("WebSocket subscribed (ID: {subscription_id})"),
        );

        // Create channel for signatures
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Spawn background task to handle incoming messages
        tokio::spawn(async move {
            while let Some(Ok(Message::Text(text))) = read.next().await {
                #[allow(clippy::collapsible_if)]
                if let Ok(notification) = serde_json::from_str::<ProgramNotification>(&text) {
                    if let Ok(sig) =
                        Signature::from_str(&notification.params.result.value.signature)
                    {
                        let _ = tx.send(sig);
                    }
                }
            }
        });

        self.state = WebSocketState::Connected {
            subscription_id,
            receiver: rx,
        };

        Ok(())
    }

    /// Ensures connection is established, reconnecting if necessary
    async fn ensure_connected(&mut self) -> Result<()> {
        match &self.state {
            WebSocketState::Disconnected => {
                self.connect().await?;
            }
            WebSocketState::Connected { receiver, .. } => {
                // Check if receiver is still alive
                if receiver.is_closed() {
                    use crate::common::logging;
                    logging::log(
                        logging::LogLevel::Warning,
                        "WebSocket disconnected, reconnecting...",
                    );
                    sleep(Duration::from_secs(self.reconnect_delay_secs)).await;
                    self.state = WebSocketState::Disconnected;
                    self.connect().await?;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TransactionSource for WebSocketSource {
    async fn next_batch(&mut self) -> Result<Vec<Signature>> {
        self.ensure_connected().await?;

        match &mut self.state {
            WebSocketState::Connected { receiver, .. } => {
                let mut signatures = Vec::new();

                // Wait for at least one signature
                if let Some(sig) = receiver.recv().await {
                    signatures.push(sig);

                    // Collect any additional signatures that are immediately available
                    while let Ok(sig) = receiver.try_recv() {
                        signatures.push(sig);
                        if signatures.len() >= 10 {
                            // Batch size limit
                            break;
                        }
                    }
                }

                Ok(signatures)
            }
            WebSocketState::Disconnected => Err(SolanaIndexerError::InternalError(
                "WebSocket not connected".to_string(),
            )),
        }
    }

    fn source_name(&self) -> &'static str {
        "WebSocket"
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_source_creation() {
        let ws_url = "ws://127.0.0.1:8900";
        let program_id = Pubkey::new_unique();
        let reconnect_delay = 5;

        let source = WebSocketSource::new(ws_url, program_id, reconnect_delay);

        assert_eq!(source.ws_url, ws_url);
        assert_eq!(source.program_id, program_id);
        assert_eq!(source.reconnect_delay_secs, reconnect_delay);

        match source.state {
            WebSocketState::Disconnected => assert!(true),
            _ => panic!("Expected initially disconnected state"),
        }
    }
}
