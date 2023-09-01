use ethers::types::{Log, TransactionReceipt, TransactionRequest};

pub struct LogsParams {
    pub network: String,
    pub from_block: u64,
    pub to_block: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub attempts: Option<u64>,
    pub can_fail: Option<bool>,
    pub interval: Option<u64>,
}

pub struct InterestingTransaction {
    pub bloom_id: String,
    pub transaction: TransactionRequest,
    pub receipt: Option<TransactionReceipt>,
    pub log: Option<Log>,
    pub all_logs: Option<Vec<Log>>,
}
