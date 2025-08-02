// Example usage of the state management system
// This file shows how to interact with the person storage system

use crate::state::{Person, store_person, get_age_by_name, get_person_by_name};

/// Example function showing how to use the state management
pub async fn example_usage() {
    // Store some people
    let _ = store_person("Alice".to_string(), 30);
    let _ = store_person("Bob".to_string(), 25);
    let _ = store_person("Charlie".to_string(), 35);
    
    // Query by name to get age
    if let Some(age) = get_age_by_name("Alice".to_string()) {
        println!("Alice is {} years old", age);
    }
    
    // Get complete person information
    if let Some(person) = get_person_by_name("Bob".to_string()) {
        println!("Found person: {} (age: {})", person.name, person.age);
    }
}

// Example of how you might integrate this with your Bitcoin operations
pub fn create_user_with_bitcoin_context(name: String, age: u32, bitcoin_address: String) -> Result<String, String> {
    // Store the person information
    store_person(name.clone(), age)?;
    
    // In a real application, you might also store their Bitcoin address
    // or link it to their profile in some way
    
    Ok(format!("Created user {} with Bitcoin address {}", name, bitcoin_address))
}
