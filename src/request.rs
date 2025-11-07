use std::sync::mpsc;
use uuid::Uuid;

use crate::Snapshot;
use crate::Version;

/// Requests are sent upstream. Each enum variant contains as its last value a `Sender` on
/// which the upstream will send the response.
#[derive(Debug)]
pub enum Request {
    /// Get a recent snapshot.
    GetSnapshot(mpsc::Sender<Snapshot>),

    /// Get the child version of the given version. This is used to iterate down the sequence of
    /// versions.
    GetChildVersion(Uuid, mpsc::Sender<Option<Version>>),
}
