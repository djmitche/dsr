use log::info;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::Downstream;
use crate::Request;
use crate::ServerDb;
use crate::Upstream;

pub struct CachingIntermediate {
    name: String,
    upstream: Upstream,
    downstream: Downstream,
    server_db: ServerDb,
}

impl CachingIntermediate {
    pub fn new(name: String, upstream: Upstream) -> Self {
        Self {
            name,
            upstream,
            downstream: Downstream::new(),
            server_db: ServerDb::default(),
        }
    }

    pub fn make_upstream(&mut self) -> Upstream {
        self.downstream.make_upstream()
    }

    pub fn start(self) {
        thread::spawn(move || self.run());
    }

    fn run(mut self) {
        let snapshot = self.upstream.get_snapshot();
        info!(
            "{}: Starting at snapshot version {}",
            self.name, snapshot.version_id
        );
        self.server_db.apply_snapshot(snapshot);

        loop {
            // Serve some requests
            self.downstream.serve_requests(
                |uuid| {
                    info!("{}: Got GetChildVersion request", self.name);
                    if let Some(v) = self.server_db.get_child_version(uuid) {
                        // We have the version locally, so just send that.
                        Some(v)
                    } else {
                        // We do not have the version locally, so call upstream.
                        let from_upstream = self
                            .upstream
                            .get_child_version(self.server_db.get_current_version_id());
                        // Cache the response locally
                        // TODO: also cache negative results here (None)
                        if let Some(v) = &from_upstream {
                            self.server_db.cache_version(uuid, v.clone());
                        }
                        from_upstream
                    }
                },
                || {
                    info!("{}: Got GetSnapshot request", self.name);
                    // Always send a local snapshot, rather than deferring this to
                    // upstream.
                    self.server_db.get_snapshot()
                },
                Duration::from_millis(10),
            );

            // Update to the most recent version, if there's a notice.
            if self.upstream.wait(Duration::from_millis(10)) {
                info!("{}: Intermediate updating to latest", self.name);
                loop {
                    let (tx, rx) = mpsc::channel();
                    self.upstream
                        .requests
                        .send(Request::GetChildVersion(
                            self.server_db.get_current_version_id(),
                            tx,
                        ))
                        .expect("send failed");
                    if let Some(v) = rx.recv().expect("receive failed") {
                        info!("{}: Got version {}", self.name, v.version_id);
                        self.server_db.apply_version(v);
                    } else {
                        break;
                    }
                }

                info!("{}: Sending new-version notice", self.name);
                self.downstream.notify();
            }
        }
    }
}
