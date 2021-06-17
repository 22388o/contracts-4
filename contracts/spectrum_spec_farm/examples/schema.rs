use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use spectrum_protocol::spec_farm::{
    ConfigInfo, Cw20HookMsg, HandleMsg, PoolsResponse, QueryMsg, RewardInfoResponse, StateInfo,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(ConfigInfo), &out_dir);
    export_schema(&schema_for!(HandleMsg), &out_dir);
    export_schema(&schema_for!(Cw20HookMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(PoolsResponse), &out_dir);
    export_schema(&schema_for!(RewardInfoResponse), &out_dir);
    export_schema(&schema_for!(StateInfo), &out_dir);
}
