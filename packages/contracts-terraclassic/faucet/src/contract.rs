use cosmwasm_std::{
    entry_point, to_json_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::Cw20ExecuteMsg;

use crate::error::ContractError;
use crate::msg::{
    AdminResponse, ClaimableAtResponse, ExecuteMsg, InstantiateMsg, QueryMsg, TokenConfig,
    TokensResponse,
};
use crate::state::{
    ADMIN, CLAIM_AMOUNT, COOLDOWN_SECONDS, CONTRACT_NAME, CONTRACT_VERSION, LAST_CLAIM, TOKENS,
};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;
    ADMIN.save(deps.storage, &admin)?;

    for t in &msg.tokens {
        deps.api.addr_validate(&t.address)?;
        TOKENS.save(deps.storage, &t.address, &t.decimals)?;
    }

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin", admin)
        .add_attribute("token_count", msg.tokens.len().to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim { token } => execute_claim(deps, env, info, token),
        ExecuteMsg::AddToken { token } => execute_add_token(deps, info, token),
        ExecuteMsg::RemoveToken { address } => execute_remove_token(deps, info, address),
    }
}

fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token: String,
) -> Result<Response, ContractError> {
    let decimals = TOKENS
        .may_load(deps.storage, &token)?
        .ok_or(ContractError::TokenNotRegistered {
            token: token.clone(),
        })?;

    let now = env.block.time.seconds();
    let last = LAST_CLAIM
        .may_load(deps.storage, (&info.sender, &token))?
        .unwrap_or(0);

    if last > 0 && now < last + COOLDOWN_SECONDS {
        return Err(ContractError::Cooldown {
            claimable_at: last + COOLDOWN_SECONDS,
        });
    }

    LAST_CLAIM.save(deps.storage, (&info.sender, &token), &now)?;

    let amount = Uint128::from(CLAIM_AMOUNT) * Uint128::from(10u128.pow(decimals as u32));

    let mint_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: token.clone(),
        msg: to_json_binary(&Cw20ExecuteMsg::Mint {
            recipient: info.sender.to_string(),
            amount,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(mint_msg)
        .add_attribute("action", "claim")
        .add_attribute("user", info.sender)
        .add_attribute("token", token)
        .add_attribute("amount", amount))
}

fn execute_add_token(
    deps: DepsMut,
    info: MessageInfo,
    token: TokenConfig,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized);
    }

    deps.api.addr_validate(&token.address)?;
    TOKENS.save(deps.storage, &token.address, &token.decimals)?;

    Ok(Response::new()
        .add_attribute("action", "add_token")
        .add_attribute("token", &token.address)
        .add_attribute("decimals", token.decimals.to_string()))
}

fn execute_remove_token(
    deps: DepsMut,
    info: MessageInfo,
    address: String,
) -> Result<Response, ContractError> {
    let admin = ADMIN.load(deps.storage)?;
    if info.sender != admin {
        return Err(ContractError::Unauthorized);
    }

    TOKENS.remove(deps.storage, &address);

    Ok(Response::new()
        .add_attribute("action", "remove_token")
        .add_attribute("token", &address))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ClaimableAt { user, token } => {
            to_json_binary(&query_claimable_at(deps, user, token)?)
        }
        QueryMsg::Tokens {} => to_json_binary(&query_tokens(deps)?),
        QueryMsg::Admin {} => to_json_binary(&query_admin(deps)?),
    }
}

fn query_claimable_at(deps: Deps, user: String, token: String) -> StdResult<ClaimableAtResponse> {
    let user_addr = deps.api.addr_validate(&user)?;
    let last = LAST_CLAIM
        .may_load(deps.storage, (&user_addr, &token))?
        .unwrap_or(0);

    let claimable_at = if last == 0 { 0 } else { last + COOLDOWN_SECONDS };
    Ok(ClaimableAtResponse { claimable_at })
}

fn query_tokens(deps: Deps) -> StdResult<TokensResponse> {
    let tokens: Vec<TokenConfig> = TOKENS
        .range(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|item| {
            let (address, decimals) = item?;
            Ok(TokenConfig { address, decimals })
        })
        .collect::<StdResult<_>>()?;

    Ok(TokensResponse { tokens })
}

fn query_admin(deps: Deps) -> StdResult<AdminResponse> {
    let admin = ADMIN.load(deps.storage)?;
    Ok(AdminResponse {
        admin: admin.to_string(),
    })
}
