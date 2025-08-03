use candid::CandidType;
use ic_cdk::{query, update};
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(CandidType, Clone)]
pub struct HtlcDetail {
    pub initiator_pubkey: String,
    pub time_lock: u64,
    pub secret_hash: String,
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
