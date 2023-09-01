mod contracts;
mod environment;
mod events;
mod types;

use contracts::{get_abis, holograph_addresses, ContractAbis};
use environment::Environment;
use events::{BloomFilter, BloomFilterMap, BloomType, EventType};
use types::InterestingTransaction;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::sleep;

use colored::*;
use ethers::abi::Abi;
use ethers::contract::Contract;
use ethers::prelude::*;
use ethers::types::{Address, U64};

use dotenv::dotenv;
use serde_json;

const TIMEOUT_THRESHOLD: u16 = 60_000;
const ZERO: u8 = 0;
const ONE: u8 = 1;
const TWO: u8 = 2;
const TEN: u8 = 10;

enum OperatorMode {
    Listen,
    Manual,
    Auto,
}

enum ProviderStatus {
    NotConfigured,
    Connected,
    Disconnected,
}

enum FilterType {
    To,
    From,
    FunctionSig,
    EventHash,
}

enum TransactionType {
    Unknown,
    Erc20,
    Erc721,
    Deploy,
}

struct ReplayFlag {
    replay: Option<String>, // For simplicity, use Option for optional values
}

struct ProcessBlockRange {
    process_block_range: bool,
}

struct NetworksFlag {
    networks: Vec<String>, // Vec (a dynamic array) can be used to replace JavaScript arrays
}

struct NetworkFlag {
    network: Option<String>,
}

struct TransactionFilter {
    filter_type: FilterType,
    match_field: MatchField,
    network_dependant: bool,
}
struct LogMessage {
    msg: String,
    tag_id: Option<String>,
}

enum MatchField {
    SimpleMatch(String),
    ComplexMatch(std::collections::HashMap<String, String>),
}

enum ContractType {
    ERC20,
    ERC721,
    ERC1155,
}

struct BlockJob {
    network: String,
    block: u64,
}

struct NetworkMonitor {
    networks: Vec<String>,
    providers: HashMap<String, Arc<Provider<Http>>>,
    holograph_addresses: HashMap<Environment, Address>,
    contracts: HashMap<String, ContractInstance<Arc<Provider<Http>>, Provider<Http>>>,
    current_block_height: Arc<Mutex<HashMap<String, u64>>>,
    block_jobs: Arc<Mutex<HashMap<String, Vec<BlockJob>>>>,

    bloom_filters: BloomFilterMap,
}

impl NetworkMonitor {
    fn new() -> Self {
        let addresses = holograph_addresses();

        NetworkMonitor {
            networks: vec!["optimism".to_string()], // Initialize with optimism
            providers: HashMap::new(),
            holograph_addresses: addresses,
            contracts: HashMap::new(),
            current_block_height: Arc::new(Mutex::new(HashMap::new())),
            block_jobs: Arc::new(Mutex::new(HashMap::new())),

            bloom_filters: HashMap::new(),
        }
    }

    async fn init_providers(
        &mut self,
        provider_url: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for network in &self.networks {
            let provider = Provider::<Http>::connect(provider_url).await;
            self.providers.insert(network.clone(), Arc::new(provider));
        }
        Ok(())
    }

    fn get_env() -> Result<Environment, Box<dyn std::error::Error>> {
        let env_str = std::env::var("HOLOGRAPH_ENV").unwrap_or_else(|_| "develop".to_string());
        match env_str.as_str() {
            "localhost" => Ok(Environment::Localhost),
            "experimental" => Ok(Environment::Experimental),
            "develop" => Ok(Environment::Develop),
            "testnet" => Ok(Environment::Testnet),
            "mainnet" => Ok(Environment::Mainnet),
            _ => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unsupported HOLOGRAPH_ENV value",
            ))),
        }
    }

    async fn fetch_address_from_holograph(
        &self,
        name: &str,
    ) -> Result<Address, Box<dyn std::error::Error>> {
        match self.contracts.get("holograph") {
            Some(contract) => {
                let call = contract.method::<(), Address>(name, ())?;
                call.call().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            }
            None => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Holograph contract not found",
            ))),
        }
    }

    async fn create_contract(
        &self,
        abi_str: &str,
        address: Address,
        provider: Arc<Provider<Http>>,
    ) -> Result<Contract<Provider<Http>>, Box<dyn std::error::Error>> {
        let abi: Abi = serde_json::from_str(abi_str)?;
        Ok(Contract::new(address, abi, provider))
    }

    async fn init_contracts(
        &mut self,
        env: &Environment,
        abis: &ContractAbis,
        provider_arc: &Arc<Provider<Http>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get and store the holograph contract
        let holograph_address = self.holograph_addresses.get(env).ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Holograph address not found",
            ))
        })?;
        let holograph = self
            .create_contract(abis.holograph_abi, holograph_address.clone(), provider_arc.clone())
            .await?;
        self.contracts.insert("holograph".to_string(), holograph);

        // Information for contracts we want to create and store
        let contracts_info = vec![
            ("getBridge", "bridge", &abis.holograph_bridge_abi),
            ("getFactory", "factory", &abis.holograph_factory_abi),
            ("getInterfaces", "interfaces", &abis.holograph_interfaces_abi),
            ("getRegistry", "registry", &abis.holograph_registry_abi),
            ("getOperator", "operator", &abis.holograph_operator_abi),
            // Uncomment below to add the token contract in the future
            // ("getUtilityToken", "token", &abis.holograph_token_abi),
        ];

        // Loop through contract info and fetch, create, and store each one
        for (method_name, contract_name, abi_str) in contracts_info {
            let address = self.fetch_address_from_holograph(method_name).await?;
            let abi: Abi = serde_json::from_str(abi_str)?;
            let contract = Contract::new(address, abi, provider_arc.clone());
            self.contracts.insert(contract_name.to_string(), contract);
        }

        Ok(())
    }

    async fn initialize_ethers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get the provider URL from environment variables
        let provider_url = std::env::var("PROVIDER_URL")?;

        // Initialize providers
        self.init_providers(&provider_url).await?;

        // Fetch the provider for "optimism"
        let provider_arc = self
            .providers
            .get(&"optimism".to_string())
            .ok_or_else(|| {
                Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Provider not found"))
            })
            .map(|arc| arc.clone())?;

        // Get the environment and contract abis
        let holograph_env = Self::get_env()?;
        let env_str = std::env::var("HOLOGRAPH_ENV").unwrap_or_else(|_| "develop".to_string());
        let abis = get_abis(&env_str);

        // Initialize contracts
        self.init_contracts(&holograph_env, &abis, &provider_arc).await?;

        // Print addresses directly from the contracts HashMap
        let contract_names = vec![
            "holograph",
            "bridge",
            "factory",
            "interfaces",
            "registry",
            "operator",
            // Add other contracts here
        ];

        for name in contract_names {
            if let Some(contract) = self.contracts.get(name) {
                let capitalized_name =
                    name.chars().nth(0).unwrap_or_default().to_uppercase().to_string() + &name[1..]; // Capitalize the contract name here
                self.structured_log(
                    &format!("📄 {}: {:?}", capitalized_name, contract.address()),
                    None,
                );
            }
        }

        // Get and print the messaging module address
        if let Some(operator_contract) = self.contracts.get("operator") {
            let messaging_module_address: Address =
                operator_contract.method("getMessagingModule", ())?.call().await?;
            self.structured_log(
                &format!("📄 Messaging Module: {:?}", messaging_module_address),
                None,
            );
        }

        Ok(())
    }

    // Asynchronously subscribe to a specified network.
    async fn network_subscribe(&mut self, network: &str, tx: mpsc::Sender<LogMessage>) {
        // Convert the network argument to a String.
        let network_string = network.to_string();

        // Check if there's a provider for the given network.
        if let Some(provider) = self.providers.get(&network_string) {
            // Clone the provider to use inside the async block.
            let provider_clone = provider.clone();

            // Clone the Arcs (reference-counted thread-safe smart pointers) to use inside the async block.
            let current_block_height = self.current_block_height.clone();
            let block_jobs = self.block_jobs.clone();

            // Spawn a new asynchronous task.
            tokio::spawn(async move {
                // Get an asynchronous stream of blocks from the provider.
                let mut stream =
                    provider_clone.watch_blocks().await.expect("Failed to watch blocks");

                // Initialize a mutable option for the last block number seen.
                let mut last_block: Option<u64> = None;

                // Continuously get the next block hash from the stream.
                while let Some(new_block_hash) = stream.next().await {
                    // Fetch block details using the block hash.
                    let block = provider_clone
                        .get_block(new_block_hash)
                        .await
                        .expect("Failed to get block details");

                    // Extract the block number from the block, default to 0 if not present.
                    let current_block_u64 = if let Some(actual_block) = block {
                        actual_block.number.unwrap_or(U64::from(0)).as_u64()
                    } else {
                        0
                    };

                    // If there's a previously seen block...
                    if let Some(lb) = last_block {
                        // ...and it's the same as the current block, skip processing.
                        if lb == current_block_u64 {
                            continue;
                        }

                        // If the last block seen and the current block have a gap...
                        if lb + 1 < current_block_u64 {
                            // ...log a message about the connection drop.
                            let log_msg = format!("Resuming previously dropped connection, gotta do some catching up. Block: {}", current_block_u64);
                            let _ = tx.send(LogMessage { msg: log_msg, tag_id: None }).await;

                            // Queue jobs for each missing block.
                            for block in lb + 1..current_block_u64 {
                                let mut bj = block_jobs.lock().await;
                                bj.entry(network_string.clone())
                                    .or_insert_with(Vec::new)
                                    .push(BlockJob { network: network_string.clone(), block });
                            }
                        }
                    }
                    // Update the last block to the current block.
                    last_block = Some(current_block_u64);

                    // Update the current block height in a thread-safe manner.
                    {
                        let mut cbh = current_block_height.lock().await;
                        cbh.insert(network_string.clone(), current_block_u64);
                    }

                    // Add a job for the current block.
                    {
                        let mut bj = block_jobs.lock().await;
                        bj.entry(network_string.clone()).or_insert_with(Vec::new).push(BlockJob {
                            network: network_string.clone(),
                            block: current_block_u64,
                        });
                    }

                    // Log that a new block has been mined.
                    let log_msg = format!(
                        "A new block has been mined. New block height is [{}]",
                        current_block_u64
                    );
                    let _ = tx.send(LogMessage { msg: log_msg, tag_id: None }).await;
                }
            });
        }
    }

    async fn process_block(&self, job: BlockJob) {
        let mut interesting_transactions: Vec<InterestingTransaction> = Vec::new();

        // TODO: `self.activated` is a HashMap<String, bool> to track network activation status
        // self.activated.insert(job.network.clone(), true);

        // TODO: `self.structured_log_verbose` is a method to log the current block being processed
        // self.structured_log_verbose(&job.network, "Getting block 🔍", job.block);

        if let Some(provider) = self.providers.get(&job.network) {
            let block_with_txs = provider.get_block_with_txs(U64::from(job.block)).await;

            match block_with_txs {
                Ok(Some(block)) => {
                    // Printing basic information about the block
                    println!("Block Number: {:?}", block.number);
                    println!("Block Hash: {:?}", block.hash);
                    println!("Parent Hash: {:?}", block.parent_hash);
                    println!("Number of Transactions: {}", block.transactions.len());

                    // Check if the block is recent
                    let current_height = self
                        .current_block_height
                        .lock()
                        .await
                        .get(&job.network)
                        .cloned()
                        .unwrap_or_default();
                    let is_recent_block = current_height.wrapping_sub(job.block) < 5;

                    // TODO: function update_gas_pricing to update the gas prices based on the current block
                    if is_recent_block {
                        // self.gas_prices.insert(job.network.clone(), update_gas_pricing(&job.network, &block));
                    }

                    // Check bloom logs and fetch logs if present. TODO: implement check_bloom_logs
                    // if self.check_bloom_logs(&block) {
                    //     let logs = provider.get_logs(Filter {
                    //         from_block: Some(job.block.into()),
                    //         to_block: Some(job.block.into()),
                    //         ..Default::default()
                    //     }).await;

                    //     match logs {
                    //         Ok(logs_list) => {
                    //             // TODO: sort and filter the logs and process the transactions
                    //             // self.filter_transactions2(&job, &block.transactions, &logs_list, &mut interesting_transactions);
                    //         }
                    //         Err(e) => {
                    //             // Handle error while fetching logs
                    //         }
                    //     }
                    // }

                    // If there are interesting transactions, process them
                    if !interesting_transactions.is_empty() {
                        // self.process_transactions2(&job, &interesting_transactions).await;
                    }
                }
                Ok(None) => {
                    // This case means the provider returned a successful result, but no block was found.
                    println!("No block was returned for block number {}", job.block);
                }
                Err(e) => {
                    // Handle error fetching block with transactions
                    // self.structured_log_error(&job.network, &format!("Error processing block {}", e), job.block);
                }
            }
        }

        // TODO: a block job handler to handle jobs after processing blocks
        // self.block_job_handler(&job).await;
    }

    fn build_filter(
        &self,
        bloom_type: BloomType,
        event_type: EventType,
        target_address: Option<String>,
        contract_type: Option<ContractType>,
    ) -> BloomFilter {
        // Placeholder
        vec![]
    }

    fn filter_builder(&mut self) {
        let build_event_filter =
            |event_type: EventType,
             contract_name: Option<&str>,
             contract_type: Option<ContractType>| {
                let address = contract_name
                    .and_then(|name| self.contracts.get(name))
                    .map(|contract| contract.address().to_string());

                self.build_filter(BloomType::TOPIC, event_type, address, contract_type)
            };

        // Build all filters first into a temporary vector
        let mut all_filters = vec![
            (
                EventType::TransferERC20,
                build_event_filter(EventType::TransferERC20, None, Some(ContractType::ERC20)),
            ),
            (
                EventType::TransferERC721,
                build_event_filter(EventType::TransferERC721, None, Some(ContractType::ERC721)),
            ),
            (
                EventType::TransferSingleERC1155,
                build_event_filter(
                    EventType::TransferSingleERC1155,
                    None,
                    Some(ContractType::ERC1155),
                ),
            ),
            (
                EventType::TransferBatchERC1155,
                build_event_filter(
                    EventType::TransferBatchERC1155,
                    None,
                    Some(ContractType::ERC1155),
                ),
            ),
            (
                EventType::BridgeableContractDeployed,
                build_event_filter(EventType::BridgeableContractDeployed, Some("factory"), None),
            ),
            (
                EventType::HolographableContractEvent,
                build_event_filter(EventType::HolographableContractEvent, Some("registry"), None),
            ),
            (
                EventType::CrossChainMessageSent,
                build_event_filter(EventType::CrossChainMessageSent, Some("operator"), None),
            ),
            (
                EventType::AvailableOperatorJob,
                build_event_filter(EventType::AvailableOperatorJob, Some("operator"), None),
            ),
            (
                EventType::FinishedOperatorJob,
                build_event_filter(EventType::FinishedOperatorJob, Some("operator"), None),
            ),
            (
                EventType::FailedOperatorJob,
                build_event_filter(EventType::FailedOperatorJob, Some("operator"), None),
            ),
        ];

        // Then insert filters into the hashmap
        for (event, filter) in all_filters.drain(..) {
            self.bloom_filters.insert(event, filter);
        }
    }

    // Generic retry function
    async fn retry<F, T>(
        &self,
        network: &str,
        func: F,
        attempts: usize,
        interval: u64,
    ) -> Result<T, Box<dyn std::error::Error>>
    where
        F: Fn() -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>>,
        >,
    {
        for i in 0..attempts {
            match func().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    self.structured_log_error(network, &e.to_string());
                    if i == attempts - 1 {
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!(
                                "Maximum attempts reached, function did not succeed after {} attempts",
                                attempts
                            ),
                        )));
                    }
                    sleep(Duration::from_millis(interval)).await;
                }
            }
        }
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unexpected error in retry loop",
        )))
    }

    fn structured_log(&self, msg: &str, tag_id: Option<&str>) {
        let timestamp = chrono::Utc::now().format("%+").to_string();
        let timestamp_color = "green";

        // Inferring the network from the providers.
        // For simplicity, this is just using the first provider in the providers map.
        let binding = "unknown".to_string();
        let network = self.providers.keys().next().unwrap_or(&binding);
        let network_name =
            network.chars().nth(0).unwrap_or_default().to_uppercase().to_string() + &network[1..];

        let env_name = match Self::get_env() {
            Ok(env) => format!("{:?}", env),
            Err(_) => "UnknownEnv".to_string(),
        };

        let tag_string = match tag_id {
            Some(tag) => format!("[{}] ", tag.to_string()), // Added space after the closing bracket
            None => "".to_string(),
        };

        let log_message = format!(
            "[{}] [{}] [{}] {}{}",
            timestamp.color(timestamp_color),
            network_name.color("red"),
            env_name.color("cyan"),
            tag_string,
            msg.trim_start() // Remove leading whitespaces from the message
        );

        println!("{}", log_message);
    }

    fn structured_log_error(&self, network: &str, msg: &str) {
        let timestamp = chrono::Utc::now().format("%+").to_string();
        let timestamp_color = "red"; // Changed to red for error logging

        // Using the provided network instead of inferring from the providers
        let network_name =
            network.chars().nth(0).unwrap_or_default().to_uppercase().to_string() + &network[1..];

        let env_name = match Self::get_env() {
            Ok(env) => format!("{:?}", env),
            Err(_) => "UnknownEnv".to_string(),
        };

        // For errors we're prepending the tag with [ERROR]
        let tag_string = format!("[ERROR] ");

        let log_message = format!(
            "[{}] [{}] [{}] {}{}",
            timestamp.color(timestamp_color),
            network_name.color("blue"),
            env_name.color("cyan"),
            tag_string,
            msg.trim_start() // Remove leading whitespaces from the message
        );

        println!("{}", log_message);
    }
}

fn web_socket_error_codes() -> HashMap<i32, &'static str> {
    vec![
        (1000, "Normal Closure"),
        (1001, "Going Away"),
        (1002, "Protocol Error"),
        (1003, "Unsupported Data"),
        (1004, "(For future)"),
        (1005, "No Status Received"),
        (1006, "Abnormal Closure"),
        (1007, "Invalid frame payload data"),
        (1008, "Policy Violation"),
        (1009, "Message too big"),
        (1010, "Missing Extension"),
        (1011, "Internal Error"),
        (1012, "Service Restart"),
        (1013, "Try Again Later"),
        (1014, "Bad Gateway"),
        (1015, "TLS Handshake"),
    ]
    .into_iter()
    .collect()
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); // Load environment variables from .env file
    let test_address = std::env::var("TEST_ADDRESS").expect("TEST_ADDRESS not set in environment");

    let monitor = Arc::new(Mutex::new(NetworkMonitor::new()));

    // Create a channel for log messages
    let (tx, mut rx) = mpsc::channel(32);

    {
        let mut monitor_guard = monitor.lock().await;
        if let Err(e) = monitor_guard.initialize_ethers().await {
            monitor_guard.structured_log(&format!("Error initializing Ethers: {:?}", e), None);
            return Err(e.into());
        }

        // Get the provider for the network from the monitor for other tasks to use
        let provider = match monitor_guard.providers.get("optimism") {
            Some(p) => p,
            None => {
                monitor_guard.structured_log("Couldn't find the provider for the network.", None);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Provider not found",
                )));
            }
        };

        // Start block monitoring for "optimism" network and pass the tx part of the channel
        monitor_guard.network_subscribe("optimism", tx.clone()).await;
    }

    // Dedicated task for handling log messages
    let monitor_for_task = monitor.clone();
    tokio::spawn(async move {
        while let Some(log_msg) = rx.recv().await {
            let monitor_guard = monitor_for_task.lock().await;
            monitor_guard.structured_log(&log_msg.msg, log_msg.tag_id.as_deref());
        }
    });

    // Dedicated task for processing block jobs from the shared vector
    let block_jobs_clone = monitor.lock().await.block_jobs.clone();
    let monitor_for_block_task = monitor.clone();
    tokio::spawn(async move {
        loop {
            {
                let mut block_jobs_guard = block_jobs_clone.lock().await;
                let jobs_for_network =
                    block_jobs_guard.entry("optimism".to_string()).or_insert_with(Vec::new);

                while let Some(block_job) = jobs_for_network.pop() {
                    let monitor_guard = monitor_for_block_task.lock().await;
                    monitor_guard.process_block(block_job).await;
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await; // Wait for a few seconds before checking again
        }
    });

    // Handle the Ctrl+C signal
    let ctrl_c = tokio::signal::ctrl_c();

    // This will run until a Ctrl+C signal is received.
    tokio::select! {
        _ = ctrl_c => {
            println!("\nShutting down...");
        }
        _ = async {
            // Sleep indefinitely to keep the program running
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60 * 60)).await;
            }
        } => {}
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        println!("An error occurred: {:?}", e);
    }
}
