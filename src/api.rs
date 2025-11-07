use std::sync::mpsc;
use std::time::Duration;
use uuid::Uuid;

use crate::Notice;
use crate::Request;
use crate::Snapshot;
use crate::Version;

/// Upstream represents a link to an upstream component, allowing sending requests and receiving
/// notices. This is typically acquired from an upstream's `make_upstream` and used to construct
/// a new downstream component.
pub struct Upstream {
    pub requests: mpsc::Sender<Request>,
    pub notices: bus::BusReader<Notice>,
}

impl Upstream {
    /// Call upstream to get the child version of the given version.
    pub fn get_child_version(&self, parent_version_id: Uuid) -> Option<Version> {
        let (tx, rx) = mpsc::channel();
        self.requests
            .send(Request::GetChildVersion(parent_version_id, tx))
            .expect("send failed");
        rx.recv().expect("receive failed")
    }

    /// Call upstream to get a snapshot.
    pub fn get_snapshot(&self) -> Snapshot {
        let (tx, rx) = mpsc::channel();
        self.requests
            .send(Request::GetSnapshot(tx))
            .expect("send failed");
        rx.recv().expect("receive failed")
    }

    /// Wait for a notice from upstream, for the given duration. Returns true if a notice was
    /// received.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        match self.notices.recv_timeout(timeout) {
            Ok(_) => {
                // Consume all of the notices, in case they "bunch up".
                while self.notices.try_recv().is_ok() {}
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            Err(e) => panic!("error receiving requests: {e}"),
        }
    }
}

/// Downstream represents a link to downstream components, allowing receiving requests and
/// sending notices. This is typically embedded in a component and used to interact with the
/// downstreams.
pub struct Downstream {
    requests_rx: mpsc::Receiver<Request>,
    requests_tx: mpsc::Sender<Request>,
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

    /// Create a new upstream linked to this downstream.
    pub fn make_upstream(&mut self) -> Upstream {
        Upstream {
            requests: self.requests_tx.clone(),
            notices: self.notices.add_rx(),
        }
    }

    /// Serve requests, until the timeout, calling the given functions on each request.
    pub fn serve_requests<GCV, GS>(
        &self,
        mut get_child_version: GCV,
        mut get_snapshot: GS,
        timeout: Duration,
    ) where
        GCV: FnMut(Uuid) -> Option<Version>,
        GS: FnMut() -> Snapshot,
    {
        loop {
            match self.requests_rx.recv_timeout(timeout) {
                Ok(req) => match req {
                    Request::GetChildVersion(uuid, sender) => {
                        sender.send(get_child_version(uuid)).expect("send failed");
                    }
                    Request::GetSnapshot(sender) => {
                        sender.send(get_snapshot()).expect("send failed");
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(e) => panic!("error receiving requests: {e}"),
            }
        }
    }

    /// Notify all downstreams that a new version is available.
    pub fn notify(&mut self) {
        self.notices.broadcast(Notice::NewVersion);
    }
}
