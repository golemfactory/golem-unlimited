use std::collections::HashMap;
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn generate_new_id<V>(map: &HashMap<String, V>) -> String {
    let mut id = new_id();
    while map.contains_key(&id) {
        id = new_id();
    }
    id
}
