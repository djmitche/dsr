use uuid::Uuid;

use crate::Update;

#[derive(Debug, Clone)]
pub struct Version {
    pub version_id: Uuid,
    pub updates: Vec<Update>,
}
