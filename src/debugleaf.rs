use log::info;
use std::thread;
use std::time::Duration;

use crate::ServerDb;
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
        let snapshot = self.upstream.get_snapshot();
        info!(
            "{}: Starting at snapshot version {}",
            self.name, snapshot.version_id
        );
        self.server_db.apply_snapshot(snapshot);
        loop {
            // Update to the most recent version.
            while let Some(v) = self
                .upstream
                .get_child_version(self.server_db.get_current_version_id())
            {
                info!("{}: Got version {}", self.name, v.version_id);
                self.server_db.apply_version(v);
            }

            // Wait for notice of a new version.
            while !self.upstream.wait(Duration::from_millis(1000)) {}
        }
    }
}
