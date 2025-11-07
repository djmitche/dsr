use std::sync::mpsc;

use crate::Notice;
use crate::Request;
use crate::Upstream;

/// Downstream represents a link to a downstream component, allowing receiving requests and
/// sending notices.
pub struct Downstream {
    pub requests_rx: mpsc::Receiver<Request>,
    pub requests_tx: mpsc::Sender<Request>,
    pub notices: bus::Bus<Notice>,
}

impl Downstream {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let notices = bus::Bus::new(100);
        Self {
            requests_rx: rx,
            requests_tx: tx,
            notices,
        }
    }

    pub fn make_upstream(&mut self) -> Upstream {
        Upstream {
            requests: self.requests_tx.clone(),
            notices: self.notices.add_rx(),
        }
    }
}
