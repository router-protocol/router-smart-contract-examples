use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::COUNTER;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cosmwasm_std::{to_binary, StdError, Empty, Event,
    IbcChannelOpenMsg, IbcChannelOpenResponse, Ibc3ChannelOpenResponse,
    IbcChannelCloseMsg,
    IbcBasicResponse,
    IbcChannelConnectMsg, IbcOrder, IbcPacketAckMsg, IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse,
    WasmMsg, SubMsg};
use cw2::{get_contract_version, set_contract_version};

// version info for migration info
pub const CONTRACT_NAME: &str = "counter";
pub const CONTRACT_VERSION: &str = "0.1.0";
pub const INIT_CALLBACK_ID: u64 = 7890;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    COUNTER.save(deps.storage, &0)?;
    Ok(Response::new().add_attribute("action", "counter_contract_init"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::IncreaseBy { value } => try_increase_counter(deps, env, info, value),
        ExecuteMsg::Reset {} => try_reset_counter(deps, env, info),
    }
}

fn try_increase_counter(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    value: u32,
) -> StdResult<Response> {
    let info_str: String = format!("updating counter value by {:?}", value);
    deps.api.debug(&info_str);

    let current_counter: u32 = COUNTER.load(deps.storage).unwrap();
    COUNTER.save(deps.storage, &(current_counter + value))?;
    let response = Response::new().add_attribute("value", value.to_string());
    Ok(response)
}

fn try_reset_counter(deps: DepsMut, _env: Env, _info: MessageInfo) -> StdResult<Response> {
    COUNTER.save(deps.storage, &0)?;
    let response = Response::new().add_attribute("counter_reset", "0");
    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    let ver = cw2::get_contract_version(deps.storage)?;
    // ensure we are migrating from an allowed contract
    if ver.contract != CONTRACT_NAME.to_string() {
        return Err(StdError::generic_err("Can only upgrade from same type").into());
    }
    // note: better to do proper semver compare, but string compare *usually* works
    if ver.version >= CONTRACT_VERSION.to_string() {
        return Err(StdError::generic_err("Cannot upgrade from a newer version").into());
    }

    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetContractVersion {} => to_binary(&get_contract_version(deps.storage)?),
        QueryMsg::FetchCounter {} => to_binary(&query_counter(deps)?),
    }
}

fn query_counter(deps: Deps) -> StdResult<u32> {
    return Ok(COUNTER.load(deps.storage)?);
}


#[entry_point]
/// enforces ordering and versioing constraints
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> StdResult<IbcChannelOpenResponse> {
    
    Ok(Some(Ibc3ChannelOpenResponse {
        version: CONTRACT_VERSION.to_string(),
    }))
}

#[entry_point]
/// once it's established, we create the reflect contract
pub fn ibc_channel_connect(
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> StdResult<IbcBasicResponse> {

    let msg = WasmMsg::Instantiate {
        admin: None,
        code_id: 0,
        msg: b"{}".into(),
        funds: vec![],
        label: format!("ibc-reflect-{}", 1),
    };
    let subMsg = SubMsg::reply_on_success(msg, INIT_CALLBACK_ID);

    Ok(IbcBasicResponse::new()
        .add_submessage(subMsg)
        .add_attribute("action", "ibc_connect")
        .add_attribute("channel_id", "chan_id")
        .add_event(Event::new("ibc").add_attribute("channel", "connect")))
}

#[entry_point]
/// On closed channel, we take all tokens from reflect contract to this contract.
/// We also delete the channel entry from accounts.
pub fn ibc_channel_close(
    deps: DepsMut,
    env: Env,
    msg: IbcChannelCloseMsg,
) -> StdResult<IbcBasicResponse> {

    let messages: Vec<SubMsg<Empty>> = vec![];

    Ok(IbcBasicResponse::new()
        .add_submessages(messages)
        .add_attribute("action", "ibc_close")
        .add_attribute("channel_id", "channel_id")
        .add_attribute("steal_funds", "steal_funds".to_string()))
}

#[entry_point]
/// never should be called as we do not send packets
pub fn ibc_packet_ack(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcPacketAckMsg,
) -> StdResult<IbcBasicResponse> {
    Ok(IbcBasicResponse::new().add_attribute("action", "ibc_packet_ack"))
}

#[entry_point]
/// never should be called as we do not send packets
pub fn ibc_packet_timeout(
    _deps: DepsMut,
    _env: Env,
    _msg: IbcPacketTimeoutMsg,
) -> StdResult<IbcBasicResponse> {
    Ok(IbcBasicResponse::new().add_attribute("action", "ibc_packet_timeout"))
}

// #[entry_point]
// /// we look for a the proper reflect contract to relay to and send the message
// /// We cannot return any meaningful response value as we do not know the response value
// /// of execution. We just return ok if we dispatched, error if we failed to dispatch
// pub fn ibc_packet_receive(
//     deps: DepsMut,
//     _env: Env,
//     msg: IbcPacketReceiveMsg,
// ) -> StdResult<IbcReceiveResponse> {
//     // put this in a closure so we can convert all error responses into acknowledgements
//     // (|| {
//     //     let packet = msg.packet;
//     //     // which local channel did this packet come on
//     //     let caller = packet.dest.channel_id;
//     //     let msg: PacketMsg = from_slice(&packet.data)?;
//     //     // match msg {
//     //     //     PacketMsg::Dispatch { msgs } => receive_dispatch(deps, caller, msgs),
//     //     // }
//     // })()
//     // .or_else(|e| {
//     //     // we try to capture all app-level errors and convert them into
//     //     // acknowledgement packets that contain an error code.
//     //     let acknowledgement = encode_ibc_error(format!("invalid packet: {}", e));
//     //     Ok(IbcReceiveResponse::new()
//     //         .set_ack(acknowledgement)
//     //         .add_event(Event::new("ibc").add_attribute("packet", "receive")))
//     // })

//     //let acknowledgement = encode_ibc_error(format!("invalid packet: {}", e));
//     Ok(IbcReceiveResponse::new()
//         //.set_ack("acknowledgement")
//         .add_event(Event::new("ibc").add_attribute("packet", "receive")))
// }