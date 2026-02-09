//! Log decoder registry for managing dynamic log decoders.
//!
//! The `LogDecoderRegistry` allows registering custom log decoders for specific
//! Solana programs. This enables the indexer to parse and process program-specific
//! logs and events dynamically.

use crate::types::events::ParsedEvent;
use crate::types::traits::DynamicLogDecoder;
use std::collections::HashMap;

/// Registry for managing log decoders by program ID.
///
/// This struct holds a mapping of program IDs to their respective log decoders.
/// When processing a transaction, the registry routes log events to the
/// appropriate decoders based on the program ID that emitted the log.
pub struct LogDecoderRegistry {
    decoders: HashMap<String, Vec<Box<dyn DynamicLogDecoder>>>,
}

impl LogDecoderRegistry {
    /// Creates a new, empty log decoder registry.
    ///
    /// # Returns
    ///
    /// A new `LogDecoderRegistry` instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
        }
    }

    /// Registers a log decoder for a specific program ID.
    ///
    /// This method associates a decoder with a program ID. Multiple decoders
    /// can be registered for the same program ID; they will be tried in order.
    ///
    /// # Arguments
    ///
    /// * `program_id` - The base58-encoded program ID as a string.
    /// * `decoder` - The decoder instance implementing `DynamicLogDecoder`.
    pub fn register(&mut self, program_id: String, decoder: Box<dyn DynamicLogDecoder>) {
        self.decoders.entry(program_id).or_default().push(decoder);
    }

    /// Decodes a batch of parsed events using registered decoders.
    ///
    /// This method iterates through the provided events and attempts to decode
    /// them using the decoders registered for their respective programs.
    ///
    /// # Arguments
    ///
    /// * `events` - A slice of `ParsedEvent`s to decode.
    ///
    /// # Returns
    ///
    /// A vector of decoded event data tuples: `(discriminator, data)`.
    #[must_use]
    pub fn decode_logs(&self, events: &[ParsedEvent]) -> Vec<([u8; 8], Vec<u8>)> {
        let mut decoded_events = Vec::new();

        for event in events {
            // We only look up decoders if we know the program ID
            if let Some(program_id) = &event.program_id {
                let program_id_str = program_id.to_string();

                if let Some(decoders) = self.decoders.get(&program_id_str) {
                    for decoder in decoders {
                        if let Some(decoded) = decoder.decode_log_dynamic(event) {
                            decoded_events.push(decoded);
                            break;
                        }
                    }
                }
            }
        }

        decoded_events
    }
}

impl Default for LogDecoderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::events::{EventType, ParsedEvent};
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    struct MockLogDecoder {
        should_decode: bool,
    }

    impl DynamicLogDecoder for MockLogDecoder {
        fn decode_log_dynamic(&self, _event: &ParsedEvent) -> Option<([u8; 8], Vec<u8>)> {
            if self.should_decode {
                Some(([1; 8], vec![1, 2, 3]))
            } else {
                None
            }
        }
    }

    #[test]
    fn test_log_registry_creation() {
        let registry = LogDecoderRegistry::new();
        assert!(registry.decoders.is_empty());
    }

    #[test]
    fn test_log_registry_default() {
        let registry = LogDecoderRegistry::default();
        assert!(registry.decoders.is_empty());
    }

    #[test]
    fn test_register_and_decode() {
        let mut registry = LogDecoderRegistry::new();
        let program_id_str = "11111111111111111111111111111111";
        let program_id = Pubkey::from_str(program_id_str).unwrap();

        // Register a decoder that succeeds
        registry.register(
            program_id_str.to_string(),
            Box::new(MockLogDecoder {
                should_decode: true,
            }),
        );

        // Create a matching event
        let event = ParsedEvent {
            event_type: EventType::ProgramLog,
            program_id: Some(program_id),
            data: Some("test log".to_string()),
        };

        // Test successful decoding
        let results = registry.decode_logs(std::slice::from_ref(&event));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, vec![1, 2, 3]);

        // Test unsuccessful decoding (decoder returns None)
        registry.decoders.clear();
        registry.register(
            program_id_str.to_string(),
            Box::new(MockLogDecoder {
                should_decode: false,
            }),
        );
        let results = registry.decode_logs(std::slice::from_ref(&event));
        assert!(results.is_empty());
    }

    #[test]
    fn test_decode_no_matching_program() {
        let mut registry = LogDecoderRegistry::new();
        let program_id_str = "11111111111111111111111111111111";
        registry.register(
            program_id_str.to_string(),
            Box::new(MockLogDecoder {
                should_decode: true,
            }),
        );

        // Event from different program (Token Program)
        let other_program =
            Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();
        let event = ParsedEvent {
            event_type: EventType::ProgramLog,
            program_id: Some(other_program),
            data: Some("test log".to_string()),
        };

        let results = registry.decode_logs(std::slice::from_ref(&event));
        assert!(results.is_empty());
    }
}
