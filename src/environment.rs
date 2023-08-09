#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub enum Environment {
    Localhost,
    Experimental,
    Develop,
    Testnet,
    Mainnet,
}
