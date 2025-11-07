use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct Snapshot {
    pub version_id: Uuid,
    pub data: HashMap<String, String>,
}
