use std::collections::HashMap;

pub type BloomFilter = Vec<u8>; // Placeholder type. This should be replaced with the actual data type for a bloom filter in Rust.
pub type BloomFilterMap = HashMap<EventType, BloomFilter>;

use ethers::abi::Abi; // This is the closest thing to the `Interface` in ethers.js
use ethers::types::H256;
use ethers::types::{Log, U256};
use ethers::utils::id;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum EventType {
    UNKNOWN,
    TBD,
    TransferERC20,
    HolographableTransferERC20,
    TransferERC721,
    HolographableTransferERC721,
    TransferSingleERC1155,
    HolographableTransferSingleERC1155,
    TransferBatchERC1155,
    HolographableTransferBatchERC1155,
    BridgeableContractDeployed,
    CrossChainMessageSent,
    AvailableOperatorJob,
    FinishedOperatorJob,
    FailedOperatorJob,
    PacketLZ,
    V1PacketLZ,
    TestLzEvent,
    HolographableContractEvent,
}

pub struct BaseEvent {
    event_type: EventType,
    contract: String,
    log_index: u32, // Equivalent to `number` in TypeScript for non-negative integers
}

pub struct HolographableContractEvent {
    base: BaseEvent,
    contract_address: String,
    payload: String,
}

pub struct TransferERC20Event {
    base: BaseEvent,
    from: String,
    to: String,
    value: U256, // Equivalent to `BigNumber` in TypeScript
}

pub struct TransferERC721Event {
    base: BaseEvent,
    from: String,
    to: String,
    token_id: U256,
}

pub struct TransferSingleERC1155Event {
    base: BaseEvent,
    operator: String,
    from: String,
    to: String,
    token_id: U256,
    value: U256,
}

pub struct TransferBatchERC1155Event {
    base: BaseEvent,
    operator: String,
    from: String,
    to: String,
    token_ids: Vec<U256>, // Equivalent to `BigNumber[]` in TypeScript
    values: Vec<U256>,
}

pub struct BridgeableContractDeployedEvent {
    base: BaseEvent,
    contract_address: String,
    hash: String,
}

pub struct CrossChainMessageSentEvent {
    base: BaseEvent,
    message_hash: String,
}

pub struct AvailableOperatorJobEvent {
    base: BaseEvent,
    job_hash: String,
    payload: String,
}

pub struct FinishedOperatorJobEvent {
    base: BaseEvent,
    job_hash: String,
    operator: String,
}

pub struct FailedOperatorJobEvent {
    base: BaseEvent,
    job_hash: String,
}

struct Event {
    event_type: EventType,
    sig_hash: String,
    custom_sig_hash: Option<String>,
    name: String,
    event_name: String,
    event: String,
}

pub enum BloomType {
    UNKNOWN,
    TOPIC,
    CONTRACT,
    ADDRESS,
}

fn get_iface() -> Abi {
    Abi::default()
}
