#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Binary, CanonicalAddr, Decimal, Deps, DepsMut, Env, MessageInfo,
    Order, Response, StdError, StdResult, Uint128,
};

use crate::{
    bond::bond,
    compound::{compound, stake},
    state::{read_config, state_store, store_config, Config, PoolInfo, State},
};

use cw20::Cw20ReceiveMsg;

use crate::bond::{deposit_spec_reward, query_reward_info, unbond, withdraw};
use crate::state::{pool_info_read, pool_info_store, read_state};
use spectrum_protocol::anchor_farm::{
    ConfigInfo, Cw20HookMsg, ExecuteMsg, MigrateMsg, PoolItem, PoolsResponse, QueryMsg, StateInfo,
};

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
    env: Env,
    _info: MessageInfo,
    msg: ConfigInfo,
) -> StdResult<Response> {
    validate_percentage(msg.community_fee, "community_fee")?;
    validate_percentage(msg.platform_fee, "platform_fee")?;
    validate_percentage(msg.controller_fee, "controller_fee")?;

    let api = deps.api;
    store_config(
        deps.storage,
        &Config {
            owner: deps.api.addr_canonicalize(&msg.owner)?,
            terraswap_factory: deps.api.addr_canonicalize(&msg.terraswap_factory)?,
            spectrum_token: deps.api.addr_canonicalize(&msg.spectrum_token)?,
            spectrum_gov: deps.api.addr_canonicalize(&msg.spectrum_gov)?,
            anchor_token: deps.api.addr_canonicalize(&msg.anchor_token)?,
            anchor_staking: deps.api.addr_canonicalize(&msg.anchor_staking)?,
            anchor_gov: deps.api.addr_canonicalize(&msg.anchor_gov)?,
            platform: if let Some(platform) = msg.platform {
                api.addr_canonicalize(&platform)?
            } else {
                CanonicalAddr::from(vec![])
            },
            controller: if let Some(controller) = msg.controller {
                api.addr_canonicalize(&controller)?
            } else {
                CanonicalAddr::from(vec![])
            },
            base_denom: msg.base_denom,
            community_fee: msg.community_fee,
            platform_fee: msg.platform_fee,
            controller_fee: msg.controller_fee,
            deposit_fee: msg.deposit_fee,
            lock_start: msg.lock_start,
            lock_end: msg.lock_end,
        },
    )?;

    state_store(deps.storage).save(&State {
        contract_addr: deps.api.addr_canonicalize(&env.contract.address.as_str())?,
        previous_spec_share: Uint128::zero(),
        spec_share_index: Decimal::zero(),
        total_farm_share: Uint128::zero(),
        total_weight: 0u32,
        earning: Uint128::zero(),
    })?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::update_config {
            owner,
            platform,
            controller,
            community_fee,
            platform_fee,
            controller_fee,
            deposit_fee,
            lock_start,
            lock_end,
        } => update_config(
            deps,
            env,
            info,
            owner,
            platform,
            controller,
            community_fee,
            platform_fee,
            controller_fee,
            deposit_fee,
            lock_start,
            lock_end,
        ),
        ExecuteMsg::register_asset {
            asset_token,
            staking_token,
            weight,
            auto_compound,
        } => register_asset(
            deps,
            env,
            info,
            asset_token,
            staking_token,
            weight,
            auto_compound,
        ),
        ExecuteMsg::unbond {
            asset_token,
            amount,
        } => unbond(deps, env, info, asset_token, amount),
        ExecuteMsg::withdraw { asset_token } => withdraw(deps, env, info, asset_token),
        ExecuteMsg::stake { asset_token } => stake(deps, env, info, asset_token),
        ExecuteMsg::compound {} => compound(deps, env, info),
    }
}

fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    match from_binary(&cw20_msg.msg) {
        Ok(Cw20HookMsg::bond {
            staker_addr,
            asset_token,
            compound_rate,
        }) => bond(
            deps,
            env,
            info,
            staker_addr.unwrap_or(cw20_msg.sender),
            asset_token,
            cw20_msg.amount,
            compound_rate,
        ),
        Err(_) => Err(StdError::generic_err("data should be given")),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<String>,
    platform: Option<String>,
    controller: Option<String>,
    community_fee: Option<Decimal>,
    platform_fee: Option<Decimal>,
    controller_fee: Option<Decimal>,
    deposit_fee: Option<Decimal>,
    lock_start: Option<u64>,
    lock_end: Option<u64>,
) -> StdResult<Response> {
    let mut config: Config = read_config(deps.storage)?;

    if deps.api.addr_canonicalize(&info.sender.as_str())? != config.owner {
        return Err(StdError::generic_err("unauthorized"));
    }

    if let Some(owner) = owner {
        config.owner = deps.api.addr_canonicalize(&owner)?;
    }

    if let Some(platform) = platform {
        config.platform = deps.api.addr_canonicalize(&platform)?;
    }

    if let Some(controller) = controller {
        config.controller = deps.api.addr_canonicalize(&controller)?;
    }

    if let Some(community_fee) = community_fee {
        validate_percentage(community_fee, "community_fee")?;
        config.community_fee = community_fee;
    }

    if let Some(platform_fee) = platform_fee {
        validate_percentage(platform_fee, "platform_fee")?;
        config.platform_fee = platform_fee;
    }

    if let Some(controller_fee) = controller_fee {
        validate_percentage(controller_fee, "controller_fee")?;
        config.controller_fee = controller_fee;
    }

    if let Some(deposit_fee) = deposit_fee {
        validate_percentage(deposit_fee, "deposit_fee")?;
        config.deposit_fee = deposit_fee;
    }

    if let Some(lock_start) = lock_start {
        config.lock_start = lock_start;
    }

    if let Some(lock_end) = lock_end {
        config.lock_end = lock_end;
    }

    store_config(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![attr("action", "update_config")]))
}

pub fn register_asset(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    asset_token: String,
    staking_token: String,
    weight: u32,
    auto_compound: bool,
) -> StdResult<Response> {
    let config: Config = read_config(deps.storage)?;
    let asset_token_raw = deps.api.addr_canonicalize(&asset_token)?;

    if config.owner != deps.api.addr_canonicalize(&info.sender.as_str())? {
        return Err(StdError::generic_err("unauthorized"));
    }

    let pool_count = pool_info_read(deps.storage)
        .range(None, None, Order::Descending)
        .count();

    if pool_count >= 1 {
        return Err(StdError::generic_err("Already registered one asset"));
    }

    let mut state = read_state(deps.storage)?;
    deposit_spec_reward(deps.as_ref(), &mut state, &config, env.block.height, false)?;

    let mut pool_info = pool_info_read(deps.storage)
        .may_load(asset_token_raw.as_slice())?
        .unwrap_or_else(|| PoolInfo {
            staking_token: deps.api.addr_canonicalize(&staking_token).unwrap(),
            total_auto_bond_share: Uint128::zero(),
            total_stake_bond_share: Uint128::zero(),
            total_stake_bond_amount: Uint128::zero(),
            weight: 0u32,
            auto_compound: false,
            farm_share: Uint128::zero(),
            farm_share_index: Decimal::zero(),
            state_spec_share_index: state.spec_share_index,
            auto_spec_share_index: Decimal::zero(),
            stake_spec_share_index: Decimal::zero(),
            reinvest_allowance: Uint128::zero(),
        });
    state.total_weight = state.total_weight + weight - pool_info.weight;
    pool_info.weight = weight;
    pool_info.auto_compound = auto_compound;

    pool_info_store(deps.storage).save(&asset_token_raw.as_slice(), &pool_info)?;
    state_store(deps.storage).save(&state)?;
    Ok(Response::new().add_attributes(vec![
        attr("action", "register_asset"),
        attr("asset_token", asset_token.as_str()),
    ]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::config {} => to_binary(&query_config(deps)?),
        QueryMsg::pools {} => to_binary(&query_pools(deps)?),
        QueryMsg::reward_info {
            staker_addr,
            height,
        } => to_binary(&query_reward_info(deps, staker_addr, height)?),
        QueryMsg::state {} => to_binary(&query_state(deps)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigInfo> {
    let config = read_config(deps.storage)?;
    let resp = ConfigInfo {
        owner: deps.api.addr_humanize(&config.owner)?.to_string(),
        terraswap_factory: deps
            .api
            .addr_humanize(&config.terraswap_factory)?
            .to_string(),
        spectrum_token: deps.api.addr_humanize(&config.spectrum_token)?.to_string(),
        anchor_token: deps.api.addr_humanize(&config.anchor_token)?.to_string(),
        anchor_staking: deps.api.addr_humanize(&config.anchor_staking)?.to_string(),
        spectrum_gov: deps.api.addr_humanize(&config.spectrum_gov)?.to_string(),
        anchor_gov: deps.api.addr_humanize(&config.anchor_gov)?.to_string(),
        platform: if config.platform == CanonicalAddr::from(vec![]) {
            None
        } else {
            Some(deps.api.addr_humanize(&config.platform)?.to_string())
        },
        controller: if config.controller == CanonicalAddr::from(vec![]) {
            None
        } else {
            Some(deps.api.addr_humanize(&config.controller)?.to_string())
        },
        base_denom: config.base_denom,
        community_fee: config.community_fee,
        platform_fee: config.platform_fee,
        controller_fee: config.controller_fee,
        deposit_fee: config.deposit_fee,
        lock_start: config.lock_start,
        lock_end: config.lock_end,
    };

    Ok(resp)
}

fn query_pools(deps: Deps) -> StdResult<PoolsResponse> {
    let pools = pool_info_read(deps.storage)
        .range(None, None, Order::Descending)
        .map(|item| {
            let (asset_token, pool_info) = item?;
            Ok(PoolItem {
                asset_token: deps
                    .api
                    .addr_humanize(&CanonicalAddr::from(asset_token))?
                    .to_string(),
                staking_token: deps
                    .api
                    .addr_humanize(&pool_info.staking_token)?
                    .to_string(),
                weight: pool_info.weight,
                auto_compound: pool_info.auto_compound,
                total_auto_bond_share: pool_info.total_auto_bond_share,
                total_stake_bond_share: pool_info.total_stake_bond_share,
                total_stake_bond_amount: pool_info.total_stake_bond_amount,
                farm_share: pool_info.farm_share,
                state_spec_share_index: pool_info.state_spec_share_index,
                farm_share_index: pool_info.farm_share_index,
                stake_spec_share_index: pool_info.stake_spec_share_index,
                auto_spec_share_index: pool_info.auto_spec_share_index,
                reinvest_allowance: pool_info.reinvest_allowance,
            })
        })
        .collect::<StdResult<Vec<PoolItem>>>()?;
    Ok(PoolsResponse { pools })
}

fn query_state(deps: Deps) -> StdResult<StateInfo> {
    let state = read_state(deps.storage)?;
    Ok(StateInfo {
        spec_share_index: state.spec_share_index,
        previous_spec_share: state.previous_spec_share,
        total_farm_share: state.total_farm_share,
        total_weight: state.total_weight,
    })
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}
