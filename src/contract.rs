use snafu::ResultExt;

use cosmwasm::errors::{Result, SerializeErr, unauthorized};
use cosmwasm::serde::to_vec;
use cosmwasm::traits::{Api, Extern, Storage};
use cosmwasm::types::{Params, Response, CosmosMsg, HumanAddr};

use crate::msg::{HandleMsg, InitMsg, QueryMsg, OwnerResponse};
use crate::state::{config, config_read, State};

pub fn init<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    _msg: InitMsg,
) -> Result<Response> {
    let state = State {
        owner: params.message.signer,
    };

    config(&mut deps.storage).save(&state)?;

    Ok(Response::default())
}

pub fn handle<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: HandleMsg,
) -> Result<Response> {
    match msg {
        HandleMsg::ReflectMsg { msg} => try_reflect(deps, params, msg),
        HandleMsg::ChangeOwner { owner } => try_change_owner(deps, params, owner),
    }
}

pub fn try_reflect<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    msg: CosmosMsg,
) -> Result<Response> {
    let state = config(&mut deps.storage).load()?;
    if params.message.signer != state.owner {
        return unauthorized();
    }
    let res = Response {
        messages: vec![msg],
        log: None,
        data: None,
    };
    Ok(res)
}

pub fn try_change_owner<S: Storage, A: Api>(
    deps: &mut Extern<S, A>,
    params: Params,
    owner: HumanAddr,
) -> Result<Response> {
    let api = deps.api;
    config(&mut deps.storage).update(&|mut state| {
        if params.message.signer != state.owner {
            return unauthorized();
        }
        state.owner = api.canonical_address(&owner)?;
        Ok(state)
    })?;
    Ok(Response::default())
}

pub fn query<S: Storage, A: Api>(deps: &Extern<S, A>, msg: QueryMsg) -> Result<Vec<u8>> {
    match msg {
        QueryMsg::GetOwner {} => query_owner(deps),
    }
}

fn query_owner<S: Storage, A: Api>(deps: &Extern<S, A>) -> Result<Vec<u8>> {
    let state = config_read(&deps.storage).load()?;

    let resp = OwnerResponse {
        owner: deps.api.human_address(&state.owner)?,
    };
    to_vec(&resp).context(SerializeErr {
        kind: "OwnerResponse",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::errors::Error;
    use cosmwasm::mock::{dependencies, mock_params};
    use cosmwasm::serde::from_slice;
    use cosmwasm::types::coin;

    #[test]
    fn proper_initialization() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(&deps.api, "creator", &coin("1000", "earth"), &[]);

        // we can just call .unwrap() to assert this was a success
        let res = init(&mut deps, params, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(
            &deps.api,
            "creator",
            &coin("2", "token"),
            &coin("2", "token"),
        );
        let _res = init(&mut deps, params, msg).unwrap();

        // beneficiary can release it
        let params = mock_params(&deps.api, "anyone", &coin("2", "token"), &[]);
        let msg = HandleMsg::Increment {};
        let _res = handle(&mut deps, params, msg).unwrap();

        // should increase counter by 1
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = dependencies(20);

        let msg = InitMsg { count: 17 };
        let params = mock_params(
            &deps.api,
            "creator",
            &coin("2", "token"),
            &coin("2", "token"),
        );
        let _res = init(&mut deps, params, msg).unwrap();

        // beneficiary can release it
        let unauth_params = mock_params(&deps.api, "anyone", &coin("2", "token"), &[]);
        let msg = HandleMsg::Reset { count: 5 };
        let res = handle(&mut deps, unauth_params, msg);
        match res {
            Err(Error::Unauthorized { .. }) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_params = mock_params(&deps.api, "creator", &coin("2", "token"), &[]);
        let msg = HandleMsg::Reset { count: 5 };
        let _res = handle(&mut deps, auth_params, msg).unwrap();

        // should now be 5
        let res = query(&deps, QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_slice(&res).unwrap();
        assert_eq!(5, value.count);
    }
}
