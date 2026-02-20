use cosmwasm_schema::{cw_serde, QueryResponses};

#[cw_serde]
pub struct TokenConfig {
    /// CW20 contract address
    pub address: String,
    /// Token decimals (e.g. 6 for uluna-scale, 18 for EVM-scale)
    pub decimals: u8,
}

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin who can add/remove tokens
    pub admin: String,
    /// Initial set of claimable tokens
    pub tokens: Vec<TokenConfig>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Claim 10 tokens (rate-limited to once per 24h per wallet per token)
    Claim { token: String },
    /// Admin: register a new claimable token
    AddToken { token: TokenConfig },
    /// Admin: remove a token from the faucet
    RemoveToken { address: String },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the timestamp (seconds) when the user can next claim this token.
    /// Returns 0 if the user has never claimed.
    #[returns(ClaimableAtResponse)]
    ClaimableAt { user: String, token: String },
    /// Returns all registered tokens
    #[returns(TokensResponse)]
    Tokens {},
    /// Returns the admin address
    #[returns(AdminResponse)]
    Admin {},
}

#[cw_serde]
pub struct ClaimableAtResponse {
    pub claimable_at: u64,
}

#[cw_serde]
pub struct TokensResponse {
    pub tokens: Vec<TokenConfig>,
}

#[cw_serde]
pub struct AdminResponse {
    pub admin: String,
}
