#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Binary, CanonicalAddr, Decimal, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdError, StdResult, Uint128,
};

use crate::{
    state::{read_config, store_config, Config},
    proxy::{query_staker_info_gov}
};

use cw20::Cw20ReceiveMsg;

use spectrum_protocol::gov_proxy::{
    ConfigInfo, Cw20HookMsg, ExecuteMsg, MigrateMsg, QueryMsg, StateInfo,
};
use spectrum_protocol::wallet::ExecuteMsg::unstake;
use crate::proxy::stake;

/// (we require 0-1)
fn validate_percentage(value: Decimal, field: &str) -> StdResult<()> {
    if value > Decimal::one() {
        Err(StdError::generic_err(field.to_string() + " must be 0 to 1"))
    } else {
        Ok(())
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ConfigInfo,
) -> StdResult<Response> {
    store_config(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            farm_contract: if let Some(farm_contract) = msg.farm_contract {
                Some(deps.api.addr_canonicalize(&farm_contract)?)
            } else {
                None
            },
            farm_token: deps.api.addr_canonicalize(&msg.farm_token)?,
            farm_gov: deps.api.addr_canonicalize(&msg.farm_gov)?,
            spectrum_gov: deps.api.addr_canonicalize(&msg.spectrum_gov)?
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::UpdateConfig {
            owner,
            farm_contract
        } => update_config(
            deps,
            info,
            owner,
            farm_contract,
        ),
        // ExecuteMsg::Unstake { amount} => unstake () TODO
    }
}

fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::Stake {}) => stake(
            deps,
            env,
            info,
            cw20_msg.sender,
            cw20_msg.amount,
        ),
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

// Deployment sequence
// 1.gov_proxy without farm contract
// 2.farm contract with gov_proxy address
// 3.update_config gov_proxy to add farm contract
#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    farm_contract: Option<String>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        if config.owner == config.spectrum_gov {
            return Err(StdError::generic_err("cannot update owner"));
        }
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(farm_contract) = farm_contract {
        config.farm_contract = Option::from(deps.api.addr_canonicalize(&farm_contract)?);
    }

    store_config(deps.storage, &config)?;
    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::StakerInfo {
            staker_addr,
        } => to_binary(&query_staker_info_gov(deps, env, staker_addr)?)
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigInfo> {
    let config = read_config(deps.storage)?;
    let resp = ConfigInfo {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        farm_contract: if let Some(farm_contract) = config.farm_contract {
            Some(deps.api.addr_humanize(&farm_contract)?.to_string())
        } else {
            None
        },
        farm_token: deps.api.addr_humanize(&config.farm_token)?.to_string(),
        farm_gov: deps.api.addr_humanize(&config.farm_gov)?.to_string(),
        spectrum_gov: deps.api.addr_humanize(&config.spectrum_gov)?.to_string(),
    };

    Ok(resp)
}

fn query_state(deps: Deps) -> StdResult<StateInfo> {
    let state = read_state(deps.storage)?;
    Ok(StateInfo {
        total_deposit: state.total_deposit,
        total_withdraw: state.total_withdraw,
        token_gain: Default::default()
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
