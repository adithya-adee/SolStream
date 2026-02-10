use honggfuzz::fuzz;
use serde_json::json;
use solana_indexer::Decoder;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
    EncodedTransactionWithStatusMeta, UiInstruction, UiMessage, UiParsedInstruction,
    UiParsedMessage, UiTransaction, UiTransactionStatusMeta, option_serializer::OptionSerializer,
    parse_instruction::ParsedInstruction,
};

fn create_mock_transaction(
    instructions: Vec<UiInstruction>,
) -> EncodedConfirmedTransactionWithStatusMeta {
    EncodedConfirmedTransactionWithStatusMeta {
        slot: 123456,
        block_time: Some(1678888888),
        transaction: EncodedTransactionWithStatusMeta {
            version: None,
            transaction: EncodedTransaction::Json(UiTransaction {
                signatures: vec!["sig1".to_string()],
                message: UiMessage::Parsed(UiParsedMessage {
                    account_keys: vec![],
                    recent_blockhash: "hash".to_string(),
                    instructions,
                    address_table_lookups: None,
                }),
            }),
            meta: Some(UiTransactionStatusMeta {
                err: None,
                status: Ok(()),
                fee: 5000,
                pre_balances: vec![],
                post_balances: vec![],
                inner_instructions: OptionSerializer::None,
                log_messages: OptionSerializer::None,
                pre_token_balances: OptionSerializer::None,
                post_token_balances: OptionSerializer::None,
                rewards: OptionSerializer::None,
                loaded_addresses: OptionSerializer::None,
                return_data: OptionSerializer::None,
                compute_units_consumed: OptionSerializer::None,
            }),
        },
    }
}

fn main() {
    let decoder = Decoder::new();
    loop {
        fuzz!(|data: &[u8]| {
            // Scenario 1: Fuzz with random transaction structure (via serde)
            if let Ok(tx) =
                serde_json::from_slice::<EncodedConfirmedTransactionWithStatusMeta>(data)
            {
                let _ = decoder.decode_transaction(&tx);
            }

            // Scenario 2: Fuzz with valid structure but random instruction data
            // Use the first byte to determine which scenario to run, or just run both/mix
            if data.len() > 1 {
                let instruction =
                    UiInstruction::Parsed(UiParsedInstruction::Parsed(ParsedInstruction {
                        program: "system".to_string(),
                        program_id: "11111111111111111111111111111111".to_string(),
                        parsed: json!({"type": "transfer", "data": data}),
                        stack_height: None,
                    }));
                let tx = create_mock_transaction(vec![instruction]);
                let _ = decoder.decode_transaction(&tx);
            }
        });
    }
}
