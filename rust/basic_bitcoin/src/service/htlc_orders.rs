use candid::CandidType;
use ic_cdk::{query, update};
use std::cell::RefCell;
use std::collections::HashMap;
use crate::{common::DerivationPath, ecdsa::get_ecdsa_public_key, BTC_CONTEXT};
use bitcoin::{Address, CompressedPublicKey, PublicKey, ScriptBuf, opcodes};
use bitcoin::script::PushBytesBuf;
use std::str::FromStr;
use crate::{
    common::{get_fee_per_byte},
    ecdsa::{sign_with_ecdsa},
    p2wpkh,
};
use ic_cdk::bitcoin_canister::{
    bitcoin_get_utxos, bitcoin_send_transaction, GetUtxosRequest, SendTransactionRequest,
};
use bitcoin::consensus::serialize;

#[derive(CandidType, Clone)]
pub struct HtlcDetail {
    pub initiator_pubkey: String,
    pub time_lock: u64,
    pub secret_hash: String,
    pub htlc_address: Option<String>, // P2WPKH address for this HTLC
}

#[derive(CandidType, Clone)]
struct OrderStorage {
    orders: HashMap<u64, HtlcDetail>,
    next_order_no: u64,
}

impl OrderStorage {
    fn new() -> Self {
        Self {
            orders: HashMap::new(),
            next_order_no: 1,
        }
    }
}

thread_local! {
    static STORAGE: RefCell<OrderStorage> = RefCell::new(OrderStorage::new());
}

/// Creates a new HTLC order and returns the order number
#[update]
pub fn create_order(initiator_pubkey: String, time_lock: u64, secret_hash: String) -> u64 {
    STORAGE.with(|s| {
        let mut storage = s.borrow_mut();
        let order_no = storage.next_order_no;
        
        let htlc_detail = HtlcDetail {
            initiator_pubkey,
            time_lock,
            secret_hash,
            htlc_address: None, // Address will be generated separately
        };
        
        storage.orders.insert(order_no, htlc_detail);
        storage.next_order_no += 1;
        
        order_no
    })
}

/// Retrieves a specific HTLC order by order number
#[query]
pub fn get_order(order_no: u64) -> Option<HtlcDetail> {
    STORAGE.with(|s| {
        s.borrow().orders.get(&order_no).cloned()
    })
}

/// Retrieves all HTLC orders
#[query]
pub fn get_all_orders() -> Vec<(u64, HtlcDetail)> {
    STORAGE.with(|s| {
        s.borrow().orders.iter().map(|(k, v)| (*k, v.clone())).collect()
    })
}

/// Gets the next order number that will be assigned
#[query]
pub fn get_next_order_no() -> u64 {
    STORAGE.with(|s| {
        s.borrow().next_order_no
    })
}

/// A simple greeting function for testing
#[query]
pub fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}

/// Creates a P2WPKH address for a specific HTLC order and stores it
/// Uses the order number as account number for unique derivation paths
#[update]
pub async fn get_htlc_address(order_no: u64) -> Result<String, String> {
    let ctx = BTC_CONTEXT.with(|ctx| ctx.get());
    
    // Check if the order exists
    let order_exists = STORAGE.with(|s| {
        s.borrow().orders.contains_key(&order_no)
    });
    
    if !order_exists {
        return Err(format!("Order {} does not exist", order_no));
    }
    
    // Check if address already exists for this order
    let existing_address = STORAGE.with(|s| {
        s.borrow().orders.get(&order_no).and_then(|order| order.htlc_address.clone())
    });
    
    if let Some(address) = existing_address {
        return Ok(address);
    }
    
    // Use order number as account number for unique derivation path
    // This ensures each HTLC order has a unique address
    let derivation_path = DerivationPath::p2wpkh(order_no as u32, 0);
    
    // Get the ECDSA public key for this specific derivation path
    let public_key = get_ecdsa_public_key(&ctx, derivation_path.to_vec_u8_path()).await;
    
    // Create a CompressedPublicKey from the raw public key bytes
    let public_key = CompressedPublicKey::from_slice(&public_key)
        .map_err(|e| format!("Failed to create public key: {}", e))?;
    
    // Generate a P2WPKH Bech32 address
    let address = Address::p2wpkh(&public_key, ctx.bitcoin_network).to_string();
    
    // Store the address in the HTLC order
    STORAGE.with(|s| {
        let mut storage = s.borrow_mut();
        if let Some(order) = storage.orders.get_mut(&order_no) {
            order.htlc_address = Some(address.clone());
        }
    });
    
    Ok(address)
}

/// Generates a P2WSH HTLC script
fn generate_p2wsh_htlc_script(
    payment_hash: &str,
    initiator_pubkey: &str,
    responder_pubkey: &str,
    timelock: u64,
) -> Result<ScriptBuf, String> {
    // Decode payment hash from hex
    let payment_hash_bytes = hex::decode(payment_hash)
        .map_err(|_| "Failed to decode payment hash".to_string())?;
    
    // Convert bytes to PushBytesBuf
    let mut payment_hash_buf = PushBytesBuf::new();
    for byte in payment_hash_bytes {
        payment_hash_buf.push(byte).map_err(|_| "Failed to push byte to buffer".to_string())?;
    }

    // Parse public keys
    let initiator_pubkey = PublicKey::from_str(initiator_pubkey)
        .map_err(|_| "Failed to parse initiator public key".to_string())?;
    let responder_pubkey = PublicKey::from_str(responder_pubkey)
        .map_err(|_| "Failed to parse responder public key".to_string())?;

    // Build the HTLC script
    let htlc_script = ScriptBuf::builder()
        .push_opcode(opcodes::all::OP_IF)
        .push_opcode(opcodes::all::OP_SHA256)
        .push_slice(&payment_hash_buf)
        .push_opcode(opcodes::all::OP_EQUALVERIFY)
        .push_key(&responder_pubkey)
        .push_opcode(opcodes::all::OP_CHECKSIG)
        .push_opcode(opcodes::all::OP_ELSE)
        .push_int(timelock as i64)
        .push_opcode(opcodes::all::OP_CSV)
        .push_opcode(opcodes::all::OP_DROP)
        .push_key(&initiator_pubkey)
        .push_opcode(opcodes::all::OP_CHECKSIG)
        .push_opcode(opcodes::all::OP_ENDIF)
        .into_script();

    Ok(htlc_script)
}

/// Generates a P2WSH address for HTLC
fn generate_p2wsh_htlc_address(
    payment_hash: &str,
    initiator_pubkey: &str,
    responder_pubkey: &str,
    timelock: u64,
    network: bitcoin::Network,
) -> Result<Address, String> {
    let script_buf = generate_p2wsh_htlc_script(
        payment_hash,
        initiator_pubkey,
        responder_pubkey,
        timelock,
    )?;

    let address = Address::p2wsh(&script_buf, network);
    Ok(address)
}

/// Withdraws funds from an HTLC order by creating a P2WSH HTLC address and sending funds to it
/// Takes order number, responder pubkey, and amount
#[update]
pub async fn withdraw_from_order(order_no: u64, responder_pubkey: String, amount_in_satoshi: u64) -> Result<String, String> {
    let ctx = BTC_CONTEXT.with(|ctx| ctx.get());

    if amount_in_satoshi == 0 {
        return Err("Amount must be greater than 0".to_string());
    }

    // Get the order details
    let order = STORAGE.with(|s| {
        s.borrow().orders.get(&order_no).cloned()
    });

    let order = match order {
        Some(order) => order,
        None => return Err(format!("Order {} does not exist", order_no)),
    };

    // Validate responder public key
    PublicKey::from_str(&responder_pubkey)
        .map_err(|_| "Invalid responder public key".to_string())?;

    // Generate P2WSH HTLC address
    let htlc_address = generate_p2wsh_htlc_address(
        &order.secret_hash,
        &order.initiator_pubkey,
        &responder_pubkey,
        order.time_lock,
        ctx.bitcoin_network,
    )?;

    // Get the P2WPKH address for this order (source address)
    let derivation_path = DerivationPath::p2wpkh(order_no as u32, 0);
    let own_public_key = get_ecdsa_public_key(&ctx, derivation_path.to_vec_u8_path()).await;
    let own_compressed_public_key = CompressedPublicKey::from_slice(&own_public_key)
        .map_err(|e| format!("Failed to create public key: {}", e))?;
    let own_public_key = PublicKey::from_slice(&own_public_key)
        .map_err(|e| format!("Failed to create public key: {}", e))?;
    let own_address = Address::p2wpkh(&own_compressed_public_key, ctx.bitcoin_network);

    // Get UTXOs from the order's P2WPKH address
    let own_utxos = bitcoin_get_utxos(&GetUtxosRequest {
        address: own_address.to_string(),
        network: ctx.network,
        filter: None,
    })
    .await
    .map_err(|e| format!("Failed to get UTXOs: {:?}", e))?
    .utxos;

    if own_utxos.is_empty() {
        return Err("No UTXOs available for this order".to_string());
    }

    // Build the transaction that sends `amount` to the HTLC address
    let fee_per_byte = get_fee_per_byte(&ctx).await;
    let (transaction, prevouts) = p2wpkh::build_transaction(
        &ctx,
        &own_public_key,
        &own_address,
        &own_utxos,
        &htlc_address,
        amount_in_satoshi,
        fee_per_byte,
    )
    .await;

    // Sign the transaction
    let signed_transaction = p2wpkh::sign_transaction(
        &ctx,
        &own_public_key,
        &own_address,
        transaction,
        &prevouts,
        derivation_path.to_vec_u8_path(),
        sign_with_ecdsa,
    )
    .await;

    // Send the transaction to the Bitcoin network
    bitcoin_send_transaction(&SendTransactionRequest {
        network: ctx.network,
        transaction: serialize(&signed_transaction),
    })
    .await
    .map_err(|e| format!("Failed to send transaction: {:?}", e))?;

    // Return the transaction ID
    Ok(signed_transaction.compute_txid().to_string())
}
