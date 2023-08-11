mod contracts;
mod environment;

use contracts::{get_abis, holograph_addresses, ContractAbis};
use environment::Environment;

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use ethers::abi::Abi;
use ethers::contract::Contract;
use ethers::prelude::*;
use ethers::types::Address;

use dotenv::dotenv;
use serde_json;

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

struct BlockJob {
    network: String,
    block: i64, // Assuming block numbers are i64
}

struct TransactionFilter {
    filter_type: FilterType,
    match_field: MatchField,
    network_dependant: bool,
}

enum MatchField {
    SimpleMatch(String),
    ComplexMatch(std::collections::HashMap<String, String>), // This replaces the {[key: string]: string} object structure
}

const TIMEOUT_THRESHOLD: u64 = 60_000;
const ZERO: i64 = 0;
const ONE: i64 = 1;
const TWO: i64 = 2;
const TEN: i64 = 10;

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

struct NetworkMonitor {
    networks: Vec<String>,
    providers: HashMap<String, Arc<Provider<Http>>>,
    holograph_addresses: HashMap<Environment, Address>,
    contracts: HashMap<String, ContractInstance<Arc<Provider<Http>>, Provider<Http>>>,
}

impl NetworkMonitor {
    fn new() -> Self {
        let addresses = holograph_addresses();

        NetworkMonitor {
            networks: vec!["optimism".to_string()], // Initialize with optimism
            providers: HashMap::new(),
            holograph_addresses: addresses,
            contracts: HashMap::new(),
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

        println!();
        for name in contract_names {
            if let Some(contract) = self.contracts.get(name) {
                println!("📄 {}: {:?}", name, contract.address());
            }
        }

        // Get and print the messaging module address
        if let Some(operator_contract) = self.contracts.get("operator") {
            let messaging_module_address: Address =
                operator_contract.method("getMessagingModule", ())?.call().await?;
            println!("📄 Messaging Module: {:?}", messaging_module_address);
        }

        println!();
        Ok(())
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok(); // Load environment variables from .env file
    let test_address = std::env::var("TEST_ADDRESS").expect("TEST_ADDRESS not set in environment");

    let mut monitor = NetworkMonitor::new();
    if let Err(e) = monitor.initialize_ethers().await {
        println!("Error initializing Ethers: {:?}", e);
        return Err(e.into());
    }

    // Start simple provider tests //

    // Get the provider for the network
    let provider = match monitor.providers.get("optimism") {
        Some(p) => p,
        None => {
            println!("Couldn't find the provider for the network.");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Provider not found",
            )));
        }
    };

    // Retrieve and Print the Current Block Number
    match provider.get_block_number().await {
        Ok(block_number) => println!("Current block number: {:?}", block_number),
        Err(e) => println!("Error fetching block number: {:?}", e),
    }

    // Fetch a balance
    let address = Address::from_str(&test_address).expect("invalid address");
    match provider.get_balance(address, None).await {
        Ok(balance) => println!("Balance of address {:?}: {:?}", address, balance),
        Err(e) => println!("Error fetching balance for address {:?}: {:?}", address, e),
    }

    // Fetch and Print Transaction Count
    match provider.get_transaction_count(address, None).await {
        Ok(count) => println!("Transaction count for address {:?}: {:?}", address, count),
        Err(e) => println!("Error fetching transaction count for address {:?}: {:?}", address, e),
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        println!("An error occurred: {:?}", e);
    }
}
