use std::sync::mpsc;

use crate::Notice;
use crate::Request;

/// Upstream represents a link to an upstream component, allowing sending requests and receiving
/// notices.
pub struct Upstream {
    pub requests: mpsc::Sender<Request>,
    pub notices: bus::BusReader<Notice>,
}
