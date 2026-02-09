//! Log decoder registry for managing log decoders.

use crate::types::events::ParsedEvent;
use crate::types::traits::DynamicLogDecoder;
use std::collections::HashMap;

/// Registry for managing log decoders by program ID.
pub struct LogDecoderRegistry {
    decoders: HashMap<String, Vec<Box<dyn DynamicLogDecoder>>>,
}

impl LogDecoderRegistry {
    /// Creates a new empty log decoder registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoders: HashMap::new(),
        }
    }

    /// Registers a log decoder for a specific program ID.
    pub fn register(&mut self, program_id: String, decoder: Box<dyn DynamicLogDecoder>) {
        self.decoders.entry(program_id).or_default().push(decoder);
    }

    /// Decodes all logs in a transaction.
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
