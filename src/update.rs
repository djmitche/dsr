#[derive(Debug, Clone)]
pub enum Update {
    /// Set a value in the data store, overwriting any value that may exist.
    Set(String, String),
    /// Delete a value from the data store.
    Delete(String),
}

