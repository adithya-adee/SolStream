use serde_json::json;
use solana_indexer::core::decoder::Decoder;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
    EncodedTransactionWithStatusMeta, UiInstruction, UiMessage, UiParsedInstruction,
    UiParsedMessage, UiTransaction, UiTransactionStatusMeta, option_serializer::OptionSerializer,
};
use std::time::Instant;

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
    println!("Starting Decoder Benchmark...");

    let decoder = Decoder::new();
    let iterations = 100_000;

    let start = Instant::now();

    for i in 0..iterations {
        // Create mock data using loop counter
        let data = vec![(i % 255) as u8; 32];

        let instruction = UiInstruction::Parsed(UiParsedInstruction::Parsed(
            solana_transaction_status::parse_instruction::ParsedInstruction {
                program: "system".to_string(),
                program_id: "11111111111111111111111111111111".to_string(),
                parsed: json!({"type": "transfer", "data": data}),
                stack_height: None,
            },
        ));

        let tx = create_mock_transaction(vec![instruction]);
        let _ = decoder.decode_transaction(&tx);

        if i % 10000 == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    }

    let duration = start.elapsed();
    println!("\nDecoded {} transactions in {:?}", iterations, duration);
    println!(
        "Throughput: {:.2} tx/s",
        iterations as f64 / duration.as_secs_f64()
    );
}
