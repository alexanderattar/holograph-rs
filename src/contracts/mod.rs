use crate::environment::Environment;
use ethers::types::Address;
use std::collections::HashMap;
use std::str::FromStr;

pub struct ContractAbis {
    pub cxip_erc721_abi: &'static str,
    pub faucet_abi: &'static str,
    pub holograph_abi: &'static str,
    pub holograph_bridge_abi: &'static str,
    pub holograph_drop_erc721_abi: &'static str,
    pub holograph_erc20_abi: &'static str,
    pub holograph_erc721_abi: &'static str,
    pub holograph_factory_abi: &'static str,
    pub holograph_interfaces_abi: &'static str,
    pub holograph_operator_abi: &'static str,
    pub holograph_registry_abi: &'static str,
    pub holographer_abi: &'static str,
    pub layer_zero_abi: &'static str,
    pub mock_lz_endpoint_abi: &'static str,
    pub editions_metadata_renderer_abi: &'static str,
    pub owner_abi: &'static str,
}

fn abi_path(environment: &str, contract: &str) -> &'static str {
    match environment {
        "develop" => match contract {
            "CxipERC721" => include_str!("../../abis/develop/CxipERC721.json"),
            "Faucet" => include_str!("../../abis/develop/Faucet.json"),
            "Holograph" => include_str!("../../abis/develop/Holograph.json"),
            "HolographBridge" => include_str!("../../abis/develop/HolographBridge.json"),
            "HolographDropERC721" => include_str!("../../abis/develop/HolographDropERC721.json"),
            "HolographERC20" => include_str!("../../abis/develop/HolographERC20.json"),
            "HolographERC721" => include_str!("../../abis/develop/HolographERC721.json"),
            "HolographFactory" => include_str!("../../abis/develop/HolographFactory.json"),
            "HolographInterfaces" => include_str!("../../abis/develop/HolographInterfaces.json"),
            "HolographOperator" => include_str!("../../abis/develop/HolographOperator.json"),
            "HolographRegistry" => include_str!("../../abis/develop/HolographRegistry.json"),
            "Holographer" => include_str!("../../abis/develop/Holographer.json"),
            "LayerZeroEndpointInterface" => {
                include_str!("../../abis/develop/LayerZeroEndpointInterface.json")
            }
            "MockLZEndpoint" => include_str!("../../abis/develop/MockLZEndpoint.json"),
            "EditionsMetadataRenderer" => {
                include_str!("../../abis/develop/EditionsMetadataRenderer.json")
            }
            "Owner" => include_str!("../../abis/develop/Owner.json"),

            _ => panic!("Unsupported contract"),
        },
        // Add other environments here
        _ => panic!("Unsupported environment"),
    }
}

pub fn get_abis(environment: &str) -> ContractAbis {
    ContractAbis {
        cxip_erc721_abi: abi_path(environment, "CxipERC721"),
        faucet_abi: abi_path(environment, "Faucet"),
        holograph_abi: abi_path(environment, "Holograph"),
        holograph_bridge_abi: abi_path(environment, "HolographBridge"),
        holograph_drop_erc721_abi: abi_path(environment, "HolographDropERC721"),
        holograph_erc20_abi: abi_path(environment, "HolographERC20"),
        holograph_erc721_abi: abi_path(environment, "HolographERC721"),
        holograph_factory_abi: abi_path(environment, "HolographFactory"),
        holograph_interfaces_abi: abi_path(environment, "HolographInterfaces"),
        holograph_operator_abi: abi_path(environment, "HolographOperator"),
        holograph_registry_abi: abi_path(environment, "HolographRegistry"),
        holographer_abi: abi_path(environment, "Holographer"),
        layer_zero_abi: abi_path(environment, "LayerZeroEndpointInterface"),
        mock_lz_endpoint_abi: abi_path(environment, "MockLZEndpoint"),
        editions_metadata_renderer_abi: abi_path(environment, "EditionsMetadataRenderer"),
        owner_abi: abi_path(environment, "Owner"),
    }
}

pub fn holograph_addresses() -> HashMap<Environment, Address> {
    let mut m = HashMap::new();
    m.insert(
        Environment::Localhost,
        Address::from_str("0xa3931469C1D058a98dde3b5AEc4dA002B6ca7446").expect("Invalid address"),
    );
    m.insert(
        Environment::Experimental,
        Address::from_str("0x199728d88a68856868f50FC259F01Bb4D2672Da9").expect("Invalid address"),
    );
    m.insert(
        Environment::Develop,
        Address::from_str("0x8dd0A4D129f03F1251574E545ad258dE26cD5e97").expect("Invalid address"),
    );
    m.insert(
        Environment::Testnet,
        Address::from_str("0x6429b42da2a06aA1C46710509fC96E846F46181e").expect("Invalid address"),
    );
    m.insert(
        Environment::Mainnet,
        Address::from_str("0x6429b42da2a06aA1C46710509fC96E846F46181e").expect("Invalid address"),
    );
    m
}
