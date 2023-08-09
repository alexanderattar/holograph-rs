use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use ethers::abi::Abi;
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

#[derive(Hash, Eq, PartialEq, Debug, Clone)]
enum Environment {
    Localhost,
    Experimental,
    Develop,
    Testnet,
    Mainnet,
}

struct NetworkMonitor {
    networks: Vec<String>,
    providers: HashMap<String, Arc<Provider<Http>>>,
}

const HOLOGRAPHER_ABI: &str = include_str!("../abis/Holographer.json");
const HOLOGRAPH_ABI: &str = include_str!("../abis/Holograph.json");

impl NetworkMonitor {
    fn new() -> Self {
        NetworkMonitor {
            networks: vec!["optimism".to_string()], // Initialize with optimism
            providers: HashMap::new(),
        }
    }

    fn get_holograph_addresses() -> HashMap<Environment, Address> {
        vec![
            (Environment::Localhost, "0xa3931469C1D058a98dde3b5AEc4dA002B6ca7446"),
            (Environment::Experimental, "0x199728d88a68856868f50FC259F01Bb4D2672Da9"),
            (Environment::Develop, "0x8dd0A4D129f03F1251574E545ad258dE26cD5e97"),
            (Environment::Testnet, "0x6429b42da2a06aA1C46710509fC96E846F46181e"),
            (Environment::Mainnet, "0x6429b42da2a06aA1C46710509fC96E846F46181e"),
        ]
        .into_iter()
        .map(|(env, addr_str)| (env, Address::from_str(addr_str).expect("Invalid address")))
        .collect()
    }

    async fn initialize_ethers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Get the provider URL from environment variables
        let provider_url = std::env::var("PROVIDER_URL")?;

        // Init providers
        for network in &self.networks {
            let provider = Provider::<Http>::connect(&provider_url).await;
            self.providers.insert(network.clone(), Arc::new(provider));
        }

        // Fetch the provider for "optimism"
        let provider_arc = self.providers.get(&"optimism".to_string()).ok_or_else(|| {
            Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "Provider not found"))
        })?;

        // Deserialize ABI
        let holographer_abi: Abi = serde_json::from_str(HOLOGRAPHER_ABI)?;
        let holograph_abi: Abi = serde_json::from_str(HOLOGRAPH_ABI)?;

        // Parse HOLOGRAPH_ENV from environment variable
        let env_str = std::env::var("HOLOGRAPH_ENV").unwrap_or_else(|_| "develop".to_string());
        let holograph_env = match env_str.as_str() {
            "localhost" => Environment::Localhost,
            "experimental" => Environment::Experimental,
            "develop" => Environment::Develop,
            "testnet" => Environment::Testnet,
            "mainnet" => Environment::Mainnet,
            _ => {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Unsupported HOLOGRAPH_ENV value",
                )))
            }
        };

        // Get holograph address
        let holograph_addresses = Self::get_holograph_addresses();
        let holograph_address = holograph_addresses.get(&holograph_env).ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Holograph address not found",
            ))
        })?;

        // Initialize contracts
        let holographer: ContractInstance<Arc<Provider<Http>>, _> =
            Contract::new(Address::zero(), holographer_abi.clone(), provider_arc.clone());
        let holograph =
            Contract::new(holograph_address.clone(), holograph_abi.clone(), provider_arc.clone());

        // Fetch contract details
        let bridge_address = holograph.method::<(), Address>("getBridge", ())?.call().await?;
        let factory_address = holograph.method::<(), Address>("getFactory", ())?.call().await?;
        let interfaces_address =
            holograph.method::<(), Address>("getInterfaces", ())?.call().await?;
        let registry_address = holograph.method::<(), Address>("getRegistry", ())?.call().await?;
        let token_address = holograph.method::<(), Address>("getUtilityToken", ())?.call().await?;

        // Print addresses
        let addresses = vec![
            ("Holograph", holograph_address),
            ("Bridge", &bridge_address),
            ("Factory", &factory_address),
            ("Interfaces", &interfaces_address),
            ("Registry", &registry_address),
            ("HLG Token", &token_address),
        ];

        println!();
        for (name, address) in addresses {
            println!("📄 {}: {:?}", name, address);
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
