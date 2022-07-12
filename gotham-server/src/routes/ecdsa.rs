#![allow(non_snake_case)]
// Gotham-city
//
// Copyright 2018 by Kzen Networks (kzencorp.com)
// Gotham city is free software: you can redistribute
// it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version.
//

use curv::cryptographic_primitives::proofs::sigma_dlog::*;
use curv::cryptographic_primitives::twoparty::coin_flip_optimal_rounds;
use curv::cryptographic_primitives::twoparty::dh_key_exchange_variant_with_pok_comm::{
    CommWitness, EcKeyPair, Party1FirstMessage, Party1SecondMessage,
};
use curv::elliptic::curves::secp256_k1::Secp256k1Scalar;
use curv::elliptic::curves::secp256_k1::GE;
use curv::BigInt;
use kms::chain_code::two_party as chain_code;
use kms::ecdsa::two_party::*;
use kms::rotation::two_party::party1::Rotation1;
use multi_party_ecdsa::protocols::two_party_ecdsa::lindell_2017::*;
use rocket::serde::json::Json;
use rocket::State;
use std::collections::HashMap;
use std::string::ToString;
use uuid::Uuid;

use super::super::auth::jwt::Claims;
use super::super::storage::db;
use super::super::Config;
use rusoto_dynamodb::{AttributeValue, DynamoDb, QueryInput};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct HDPos {
    pos: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct Alpha {
    value: BigInt,
}

#[derive(Debug)]
pub enum EcdsaStruct {
    KeyGenFirstMsg,
    CommWitness,
    EcKeyPair,
    PaillierKeyPair,
    Party1Private,
    Party2Public,

    PDLProver,
    PDLDecommit,
    Alpha,
    Party2PDLFirstMsg,

    CCKeyGenFirstMsg,
    CCCommWitness,
    CCEcKeyPair,
    CC,

    Party1MasterKey,

    EphEcKeyPair,
    EphKeyGenFirstMsg,

    RotateCommitMessage1M,
    RotateCommitMessage1R,
    RotateRandom1,
    RotateFirstMsg,
    RotatePrivateNew,
    RotatePdlDecom,
    RotateParty2First,
    RotateParty1Second,

    POS,
}

impl db::MPCStruct for EcdsaStruct {
    fn to_string(&self) -> String {
        format!("{:?}", self)
    }

    // backward compatibility
    fn to_table_name(&self, env: &str) -> String {
        if self.to_string() == "Party1MasterKey" {
            format!("{}_{}", env, self.to_string())
        } else {
            format!("{}-gotham-{}", env, self.to_string())
        }
    }

    fn require_customer_id(&self) -> bool {
        self.to_string() == "Party1MasterKey"
    }
}

#[post("/ecdsa/keygen/first", format = "json")]
pub async fn first_message(
    state: &State<Config>,
    claim: Claims,
) -> Result<Json<(String, party_one::KeyGenFirstMsg)>> {
    match has_active_share(&state.db, &claim.sub) {
        Err(e) => {
            let msg = format!(
                "Error when searching for active shares of customerId {}",
                &claim.sub
            );
            error!("{}: {:?}", msg, e);
            return Err(format_err!("{}", msg));
        }
        Ok(result) => {
            if result {
                let msg = format!("User {} already has an active share", &claim.sub);
                warn!("{}", msg);
                let should_fail_keygen = std::env::var("FAIL_KEYGEN_IF_ACTIVE_SHARE_EXISTS");
                if should_fail_keygen.is_ok() && should_fail_keygen.unwrap() == "true" {
                    warn!("Abort KeyGen");
                    return Err(format_err!("{}", msg));
                }
            }
        }
    }

    let (key_gen_first_msg, comm_witness, ec_key_pair) = MasterKey1::key_gen_first_message();

    let id = Uuid::new_v4().to_string();
    //save pos 0
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::POS,
        &HDPos { pos: 0u32 },
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::KeyGenFirstMsg,
        &key_gen_first_msg,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::CommWitness,
        &comm_witness,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::EcKeyPair,
        &ec_key_pair,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json((id, key_gen_first_msg)))
}

#[post("/ecdsa/keygen/<id>/second", format = "json", data = "<dlog_proof>")]
pub async fn second_message(
    state: &State<Config>,
    claim: Claims,
    id: String,
    dlog_proof: Json<DLogProof<GE>>,
) -> Result<Json<party1::KeyGenParty1Message2>, String> {
    let party2_public: GE = dlog_proof.0.pk;
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Party2Public,
        &party2_public,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    let comm_witness: party_one::CommWitness =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::CommWitness)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;
    let ec_key_pair: party_one::EcKeyPair =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::EcKeyPair)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;

    let (kg_party_one_second_message, paillier_key_pair, party_one_private) =
        MasterKey1::key_gen_second_message(comm_witness, &ec_key_pair, &dlog_proof.0);

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::PaillierKeyPair,
        &paillier_key_pair,
    )
    .await
    .or(Err("Failed to insert into db"))?;
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Party1Private,
        &party_one_private,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json(kg_party_one_second_message))
}

#[post(
    "/ecdsa/keygen/<id>/third",
    format = "json",
    data = "<party_2_pdl_first_message>"
)]
pub fn third_message(
    state: State<Config>,
    claim: Claims,
    id: String,
    party_2_pdl_first_message: Json<party_two::PDLFirstMessage>,
) -> Result<Json<(party_one::PDLFirstMessage)>> {
    let party_one_private: party_one::Party1Private =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party1Private)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let (party_one_third_message, party_one_pdl_decommit, alpha) =
        MasterKey1::key_gen_third_message(&party_2_pdl_first_message.0, &party_one_private);

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::PDLDecommit,
        &party_one_pdl_decommit,
    )?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Alpha,
        &Alpha { value: alpha },
    )?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Party2PDLFirstMsg,
        &party_2_pdl_first_message.0,
    )?;

    Ok(Json(party_one_third_message))
}

#[post(
    "/ecdsa/keygen/<id>/fourth",
    format = "json",
    data = "<party_two_pdl_second_message>"
)]
pub fn fourth_message(
    state: State<Config>,
    claim: Claims,
    id: String,
    party_two_pdl_second_message: Json<party_two::PDLSecondMessage>,
) -> Result<Json<(party_one::PDLSecondMessage)>> {
    let party_one_private: party_one::Party1Private =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party1Private)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let party_one_pdl_decommit: party_one::PDLdecommit =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::PDLDecommit)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let party_2_pdl_first_message: party_two::PDLFirstMessage =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party2PDLFirstMsg)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let alpha: Alpha = db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Alpha)?
        .ok_or(format_err!("No data for such identifier {}", id))?;

    let res = MasterKey1::key_gen_fourth_message(
        &party_2_pdl_first_message,
        &party_two_pdl_second_message.0,
        party_one_private,
        party_one_pdl_decommit,
        alpha.value,
    );

    assert!(res.is_ok());

    Ok(Json(res.unwrap()))
}

#[post("/ecdsa/keygen/<id>/chaincode/first", format = "json")]
pub async fn chain_code_first_message(
    state: &State<Config>,
    claim: Claims,
    id: String,
) -> Result<Json<Party1FirstMessage>, String> {
    let (cc_party_one_first_message, cc_comm_witness, cc_ec_key_pair1) =
        chain_code::party1::ChainCode1::chain_code_first_message();

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::CCKeyGenFirstMsg,
        &cc_party_one_first_message,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::CCCommWitness,
        &cc_comm_witness,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::CCEcKeyPair,
        &cc_ec_key_pair1,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json(cc_party_one_first_message))
}

#[post(
    "/ecdsa/keygen/<id>/chaincode/second",
    format = "json",
    data = "<cc_party_two_first_message_d_log_proof>"
)]
pub async fn chain_code_second_message(
    state: &State<Config>,
    claim: Claims,
    id: String,
    cc_party_two_first_message_d_log_proof: Json<DLogProof<GE>>,
) -> Result<Json<Party1SecondMessage<GE>>, String> {
    let cc_comm_witness: CommWitness<GE> =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::CCCommWitness)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;

    let party1_cc = chain_code::party1::ChainCode1::chain_code_second_message(
        cc_comm_witness,
        &cc_party_two_first_message_d_log_proof.0,
    );

    let party2_pub = &cc_party_two_first_message_d_log_proof.pk;
    chain_code_compute_message(state, claim, id, party2_pub).await?;

    Ok(Json(party1_cc))
}

pub async fn chain_code_compute_message(
    state: &State<Config>,
    claim: Claims,
    id: String,
    cc_party2_public: &GE,
) -> Result<Json<()>, String> {
    let cc_ec_key_pair_party1: EcKeyPair<GE> =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::CCEcKeyPair)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;
    let party1_cc = chain_code::party1::ChainCode1::compute_chain_code(
        &cc_ec_key_pair_party1,
        cc_party2_public,
    );

    db::insert(&state.db, &claim.sub, &id, &EcdsaStruct::CC, &party1_cc)
        .await
        .or(Err("Failed to insert into db"))?;
    master_key(state, claim, id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(()))
}

pub async fn master_key(
    state: &State<Config>,
    claim: Claims,
    id: String,
) -> Result<(), failure::Error> {
    let party2_public: GE = db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party2Public)
        .await?
        .ok_or_else(|| format_err!("No data for such identifier {}", id))?;

    let paillier_key_pair: party_one::PaillierKeyPair =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::PaillierKeyPair)
            .await?
            .ok_or_else(|| format_err!("No data for such identifier {}", id))?;

    let party1_cc: chain_code::party1::ChainCode1 =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::CC)
            .await?
            .ok_or_else(|| format_err!("No data for such identifier {}", id))?;

    let party_one_private: party_one::Party1Private =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party1Private)
            .await?
            .ok_or_else(|| format_err!("No data for such identifier {}", id))?;

    let comm_witness: party_one::CommWitness =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::CommWitness)
            .await?
            .ok_or_else(|| format_err!("No data for such identifier {}", id))?;

    let masterKey = MasterKey1::set_master_key(
        &party1_cc.chain_code,
        party_one_private,
        &comm_witness.public_share,
        &party2_public,
        paillier_key_pair,
    );

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Party1MasterKey,
        &masterKey,
    )
    .await
}

#[post(
    "/ecdsa/sign/<id>/first",
    format = "json",
    data = "<eph_key_gen_first_message_party_two>"
)]
pub async fn sign_first(
    state: &State<Config>,
    claim: Claims,
    id: String,
    eph_key_gen_first_message_party_two: Json<party_two::EphKeyGenFirstMsg>,
) -> Result<Json<party_one::EphKeyGenFirstMsg>, String> {
    let (sign_party_one_first_message, eph_ec_key_pair_party1) = MasterKey1::sign_first_message();

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::EphKeyGenFirstMsg,
        &eph_key_gen_first_message_party_two.0,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::EphEcKeyPair,
        &eph_ec_key_pair_party1,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json(sign_party_one_first_message))
}

// Added here because the attribute data takes only a single struct
#[derive(Serialize, Deserialize)]
pub struct SignSecondMsgRequest {
    pub message: BigInt,
    pub party_two_sign_message: party2::SignMessage,
    pub x_pos_child_key: BigInt,
    pub y_pos_child_key: BigInt,
}
#[post("/ecdsa/sign/<id>/second", format = "json", data = "<request>")]
pub async fn sign_second(
    state: &State<Config>,
    claim: Claims,
    id: String,
    request: Json<SignSecondMsgRequest>,
) -> Result<Json<party_one::SignatureRecid>, String> {
    let master_key: MasterKey1 = db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Party1MasterKey)
        .await
        .or(Err("Failed to get from db"))?
        .ok_or(format!("No data for such identifier {}", id))?;

    let x: BigInt = request.x_pos_child_key.clone();
    let y: BigInt = request.y_pos_child_key.clone();

    let child_master_key = master_key.get_child(vec![x, y]);

    let eph_ec_key_pair_party1: party_one::EphEcKeyPair =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::EphEcKeyPair)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;

    let eph_key_gen_first_message_party_two: party_two::EphKeyGenFirstMsg =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::EphKeyGenFirstMsg)
            .await
            .or(Err("Failed to get from db"))?
            .ok_or(format!("No data for such identifier {}", id))?;

    let signature_with_recid = child_master_key.sign_second_message(
        &request.party_two_sign_message,
        &eph_key_gen_first_message_party_two,
        &eph_ec_key_pair_party1,
        &request.message,
    );

    if signature_with_recid.is_err() {
        panic!("validation failed")
    };

    Ok(Json(signature_with_recid.unwrap()))
}

pub async fn get_mk(
    state: &State<Config>,
    claim: Claims,
    id: &str,
) -> Result<MasterKey1, failure::Error> {
    db::get(&state.db, &claim.sub, id, &EcdsaStruct::Party1MasterKey)
        .await?
        .ok_or_else(|| format_err!("No data for such identifier {}", id))
}

#[post("/ecdsa/rotate/<id>/first", format = "json")]
pub async fn rotate_first(
    state: &State<Config>,
    claim: Claims,
    id: String,
) -> Result<Json<coin_flip_optimal_rounds::Party1FirstMessage<GE>>, String> {
    let (party1_coin_flip_first_message, m1, r1) = Rotation1::key_rotate_first_message();
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateCommitMessage1M,
        &m1,
    )
    .await
    .or(Err("Failed to insert into db"))?;
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateCommitMessage1R,
        &r1,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json(party1_coin_flip_first_message))
}

#[post(
    "/ecdsa/rotate/<id>/second",
    format = "json",
    data = "<party2_first_message>"
)]
pub async fn rotate_second(
    state: &State<Config>,
    id: String,
    claim: Claims,
    party2_first_message: Json<coin_flip_optimal_rounds::Party2FirstMessage>,
) -> Result<
    Json<(
        coin_flip_optimal_rounds::Party1SecondMessage<GE>,
        party1::RotationParty1Message1,
    )>,
    String,
> {
    let party_one_master_key = get_mk(state, claim.clone(), &id)
        .await
        .map_err(|e| e.to_string())?;

    let m1: Secp256k1Scalar = db::get(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateCommitMessage1M,
    )
    .await
    .or(Err("Failed to get from db"))?
    .ok_or(format!("No data for such identifier {}", id))?;

    let r1: Secp256k1Scalar = db::get(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateCommitMessage1R,
    )
    .await
    .or(Err("Failed to get from db"))?
    .ok_or(format!("No data for such identifier {}", id))?;

    let (party1_second_message, random1) =
        Rotation1::key_rotate_second_message(&party2_first_message.0, &m1, &r1);
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateRandom1,
        &random1,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    let (rotation_party_one_first_message, party_one_private_new) =
        party_one_master_key.rotation_first_message(&random1);

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateFirstMsg,
        &rotation_party_one_first_message,
    )?;
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotatePrivateNew,
        &party_one_private_new,
    )?;
    Ok(Json((
        party1_second_message,
        rotation_party_one_first_message,
    )))
}

#[post(
    "/ecdsa/rotate/<id>/third",
    format = "json",
    data = "<rotation_party_two_first_message>"
)]
pub fn rotate_third(
    state: State<Config>,
    claim: Claims,
    id: String,
    rotation_party_two_first_message: Json<party_two::PDLFirstMessage>,
) -> Result<Json<(party_one::PDLFirstMessage)>> {
    let party_one_private_new: party_one::Party1Private =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotatePrivateNew)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let (rotation_party_one_second_message, party_one_pdl_decommit, alpha) =
        MasterKey1::rotation_second_message(
            &rotation_party_two_first_message.0,
            &party_one_private_new,
        );

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Alpha,
        &Alpha { value: alpha },
    )?;

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotatePdlDecom,
        &party_one_pdl_decommit,
    )?;
    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::RotateParty2First,
        &rotation_party_two_first_message.0,
    )?;
    db::insert(
        &state.db,
        &id,
        &claim.sub,
        &EcdsaStruct::RotateParty1Second,
        &rotation_party_one_second_message,
    )?;

    Ok(Json(rotation_party_one_second_message))
}

#[post(
    "/ecdsa/rotate/<id>/fourth",
    format = "json",
    data = "<rotation_party_two_second_message>"
)]
pub fn rotate_fourth(
    state: State<Config>,
    claim: Claims,
    id: String,
    rotation_party_two_second_message: Json<party_two::PDLSecondMessage>,
) -> Result<Json<(party_one::PDLSecondMessage)>> {
    let party_one_master_key = get_mk(&state, claim.clone(), &id)?;

    let rotation_party_one_first_message: party1::RotationParty1Message1 =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotateFirstMsg)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let party_one_private_new: party_one::Party1Private =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotatePrivateNew)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let random1: kms::rotation::two_party::Rotation =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotateRandom1)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let rotation_party_two_first_message: party_two::PDLFirstMessage =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotateParty2First)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let party_one_pdl_decommit: party_one::PDLdecommit =
        db::get(&state.db, &claim.sub, &id, &EcdsaStruct::RotatePdlDecom)?
            .ok_or(format_err!("No data for such identifier {}", id))?;

    let alpha: Alpha = db::get(&state.db, &claim.sub, &id, &EcdsaStruct::Alpha)?
        .ok_or(format_err!("No data for such identifier {}", id))?;

    let result_rotate_party_two_second_message = party_one_master_key.rotation_third_message(
        &rotation_party_one_first_message,
        party_one_private_new,
        &random1,
        &rotation_party_two_first_message,
        &rotation_party_two_second_message.0,
        party_one_pdl_decommit,
        alpha.value,
    );
    if result_rotate_party_two_second_message.is_err() {
        panic!("rotation failed");
    }
    let (rotation_party_one_third_message, party_one_master_key_rotated) =
        result_rotate_party_two_second_message.unwrap();

    db::insert(
        &state.db,
        &claim.sub,
        &id,
        &EcdsaStruct::Party1MasterKey,
        &party_one_master_key_rotated,
    )
    .await
    .or(Err("Failed to insert into db"))?;

    Ok(Json(rotation_party_one_third_message))
}

#[post("/ecdsa/<id>/recover", format = "json")]
pub async fn recover(
    state: &State<Config>,
    claim: Claims,
    id: String,
) -> Result<Json<u32>, String> {
    let pos_old: u32 = db::get(&state.db, &claim.sub, &id, &EcdsaStruct::POS)
        .await
        .or(Err("Failed to get from db"))?
        .ok_or(format!("No data for such identifier {}", id))?;
    Ok(Json(pos_old))
}

fn has_active_share(db: &db::DB, user_id: &str) -> Result<bool> {
    match db {
        db::DB::Local(_) => Ok(false),
        db::DB::AWS(dynamodb_client, env) => {
            let mut expression_attribute_values: HashMap<String, AttributeValue> = HashMap::new();
            expression_attribute_values.insert(
                ":customerId".into(),
                AttributeValue {
                    s: Some(user_id.to_string()),
                    ..AttributeValue::default()
                },
            );
            expression_attribute_values.insert(
                ":deleted".into(),
                AttributeValue {
                    bool: Some(true),
                    ..AttributeValue::default()
                },
            );

            let query_input = QueryInput {
                table_name: format!("{}_Party1MasterKey", env),
                projection_expression: Some("id".into()),
                key_condition_expression: Some("customerId = :customerId".into()),
                filter_expression: Some("isDeleted <> :deleted".into()),
                expression_attribute_values: Some(expression_attribute_values),
                consistent_read: Some(true),
                ..QueryInput::default()
            };
            let result = dynamodb_client.query(query_input).sync();
            match result {
                Ok(query_output) => query_output
                    .items
                    .map_or(Ok(false), |items| Ok(items.len() > 0)),
                Err(e) => Err(format_err!(
                    "Error retrieving Party1MasterKey for customerId {}: {:?}",
                    user_id,
                    e
                )),
            }
        }
    }
}
