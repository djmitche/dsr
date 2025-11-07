use log::info;
use std::sync::mpsc;
use std::thread;

use crate::Request;
use crate::ServerDb;
use crate::Snapshot;
use crate::Upstream;

pub struct DebugLeaf {
    name: String,
    upstream: Upstream,
    server_db: ServerDb,
}

impl DebugLeaf {
    pub fn new(name: String, upstream: Upstream) -> Self {
        Self {
            name,
            upstream,
            server_db: ServerDb::default(),
        }
    }

    pub fn start(self) {
        thread::spawn(move || self.run());
    }

    fn run(mut self) {
        info!("{}: Getting snapshot", self.name);
        let snapshot = self.get_snapshot();
        info!(
            "{}: Starting at snapshot version {}",
            self.name, snapshot.version_id
        );
        self.server_db.apply_snapshot(snapshot);
        loop {
            // Update to the most recent version.
            loop {
                let (tx, rx) = mpsc::channel();
                self.upstream.requests.send(Request::GetChildVersion(
                    self.server_db.get_current_version_id(),
                    tx,
                )).expect("send failed");
                if let Some(v) = rx.recv().expect("receive failed") {
                    info!("{}: Got version {}", self.name, v.version_id);
                    self.server_db.apply_version(v);
                } else {
                    break;
                }
            }

            // Wait for notice of a new version.
            self.upstream.notices.recv().expect("notice receive failed");
        }
    }

    fn get_snapshot(&mut self) -> Snapshot {
        let (tx, rx) = mpsc::channel();
        self.upstream.requests.send(Request::GetSnapshot(tx)).expect("send failed");
        rx.recv().expect("receive failed")
    }
}
