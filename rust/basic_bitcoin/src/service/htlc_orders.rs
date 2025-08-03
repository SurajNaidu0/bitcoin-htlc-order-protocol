use candid::CandidType;
use ic_cdk::{query, update};
use std::cell::RefCell;
use std::collections::HashMap;
use crate::{common::DerivationPath, ecdsa::get_ecdsa_public_key, BTC_CONTEXT};
use bitcoin::{Address, CompressedPublicKey};

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
