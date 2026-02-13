/// Rich transaction context passed to EventHandlers.
#[derive(Debug, Clone)]
pub struct TxMetadata {
    /// The slot number where the transaction was confirmed.
    pub slot: u64,
    /// The block time (Unix timestamp) if available.
    pub block_time: Option<i64>,
    /// The fee paid for the transaction in lamports.
    pub fee: u64,
    /// Account balances before the transaction.
    pub pre_balances: Vec<u64>,
    /// Account balances after the transaction.
    pub post_balances: Vec<u64>,
    /// Token balances before the transaction.
    pub pre_token_balances: Vec<TokenBalanceInfo>,
    /// Token balances after the transaction.
    pub post_token_balances: Vec<TokenBalanceInfo>,
    /// The transaction signature.
    pub signature: String,
}

/// Information about a token balance change.
#[derive(Debug, Clone)]
pub struct TokenBalanceInfo {
    /// Index of the account in the transaction's account list.
    pub account_index: u8,
    /// The mint address of the token.
    pub mint: String,
    /// The owner of the token account.
    pub owner: String,
    /// The token balance amount.
    pub amount: String, // Kept as string to match RPC JSON response usually, or parsed u64? Plan said u64.
    // RPC returns uiAmountString (String) and amount (String of u64).
    // Let's stick to String for amount to be safe with large values or decimals,
    // but plan said u64. Let's check UiTokenAmount.
    // UiTokenAmount has `amount: String`, `decimals: u8`, `uiAmount: Option<f64>`, `uiAmountString: String`.
    // The `amount` field in UiTokenAmount is a string representation of u64.
    // I will use u64 if I can parse it, or String if I want to just pass it through.
    // Implementation plan said u64. I'll try u64.
    pub decimals: u8,
    /// The programming ID (optional in some contexts but usually Token Program)
    pub program_id: Option<String>,
}
