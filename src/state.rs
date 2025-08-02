// This module demonstrates how to store and manage state in an ICP canister.
// It provides a simple example of storing person records (name + age) and querying them.

use candid::{CandidType, Deserialize};
use ic_cdk::{query, update};
use std::cell::RefCell;
use std::collections::HashMap;

/// Represents a person with their basic information.
/// This struct will be stored in the canister's state.
#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct Person {
    pub name: String,
    pub age: u32,
}

impl Person {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }
}

// Global state storage for person records.
// Uses a HashMap to map names to Person structs for efficient lookup.
// 
// Important notes about state in ICP:
// - `thread_local!` ensures state persists between function calls within the same canister instance
// - `RefCell` allows mutable access to the state in a single-threaded environment
// - State is automatically persisted across upgrades if you implement pre/post upgrade hooks
thread_local! {
    static PEOPLE: RefCell<HashMap<String, Person>> = RefCell::new(HashMap::new());
}

/// Stores a new person in the canister state.
/// If a person with the same name already exists, their age will be updated.
/// 
/// This is an `update` function because it modifies the canister state.
/// Update functions can:
/// - Modify state
/// - Take longer to execute
/// - Cost cycles to execute
/// - Are committed to the blockchain
#[update]
pub fn store_person(name: String, age: u32) -> Result<String, String> {
    // Validate input
    if name.trim().is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    
    if age > 150 {
        return Err("Age seems unrealistic (> 150)".to_string());
    }

    // Create the person struct
    let person = Person::new(name.clone(), age);
    
    // Store in the global state
    PEOPLE.with(|people| {
        let mut people_map = people.borrow_mut();
        people_map.insert(name.clone(), person);
    });
    
    Ok(format!("Successfully stored person: {} (age: {})", name, age))
}

/// Retrieves a person's age by their name.
/// Returns None if the person is not found.
/// 
/// This is a `query` function because it only reads state without modifying it.
/// Query functions:
/// - Cannot modify state
/// - Execute faster
/// - Are free to execute
/// - Don't require consensus
#[query]
pub fn get_age_by_name(name: String) -> Option<u32> {
    PEOPLE.with(|people| {
        let people_map = people.borrow();
        people_map.get(&name).map(|person| person.age)
    })
}

/// Retrieves complete person information by name.
/// Returns the full Person struct if found.
#[query]
pub fn get_person_by_name(name: String) -> Option<Person> {
    PEOPLE.with(|people| {
        let people_map = people.borrow();
        people_map.get(&name).cloned()
    })
}

/// Lists all stored people.
/// Useful for debugging or displaying all records.
#[query]
pub fn list_all_people() -> Vec<Person> {
    PEOPLE.with(|people| {
        let people_map = people.borrow();
        people_map.values().cloned().collect()
    })
}

/// Removes a person from storage by name.
/// Returns true if the person was found and removed, false otherwise.
#[update]
pub fn remove_person(name: String) -> bool {
    PEOPLE.with(|people| {
        let mut people_map = people.borrow_mut();
        people_map.remove(&name).is_some()
    })
}

/// Returns the total number of people stored.
#[query]
pub fn get_people_count() -> usize {
    PEOPLE.with(|people| {
        let people_map = people.borrow();
        people_map.len()
    })
}

/// Clears all stored people. Use with caution!
#[update]
pub fn clear_all_people() -> String {
    PEOPLE.with(|people| {
        let mut people_map = people.borrow_mut();
        let count = people_map.len();
        people_map.clear();
        format!("Cleared {} people from storage", count)
    })
}

// Advanced: State persistence across upgrades
// 
// State persistence is handled in the main lib.rs file using the existing
// pre_upgrade and post_upgrade hooks. The functions below are helper functions
// that can be called from those hooks.

/// Helper function to save people state to stable memory.
/// Call this from your main pre_upgrade hook in lib.rs.
pub fn save_people_state() {
    PEOPLE.with(|people| {
        let people_map = people.borrow();
        // Convert the HashMap to a stable format for storage
        let stable_data: Vec<(String, Person)> = people_map.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        // Store in stable memory (this is a simplified example)
        // In practice, you might want to use ic-stable-structures for more complex data
        ic_cdk::storage::stable_save((stable_data,)).expect("Failed to save people state");
    });
}

/// Helper function to restore people state from stable memory.
/// Call this from your main post_upgrade hook in lib.rs.
pub fn restore_people_state() {
    // Try to restore the state from stable memory
    if let Ok((stable_data,)) = ic_cdk::storage::stable_restore::<(Vec<(String, Person)>,)>() {
        PEOPLE.with(|people| {
            let mut people_map = people.borrow_mut();
            for (name, person) in stable_data {
                people_map.insert(name, person);
            }
        });
    }
    // If restoration fails, it just means there's no previous state to restore
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_creation() {
        let person = Person::new("Alice".to_string(), 30);
        assert_eq!(person.name, "Alice");
        assert_eq!(person.age, 30);
    }
}
