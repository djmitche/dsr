use std::sync::mpsc;
use std::thread;
use log::info;
use std::time::Duration;

use crate::Upstream;
use crate::Snapshot;
use crate::Downstream;
use crate::ServerDb;
use crate::Request;
use crate::Notice;

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
        let snapshot = self.get_snapshot();
        info!(
            "{}: Starting at snapshot version {}",
            self.name, snapshot.version_id
        );
        self.server_db.apply_snapshot(snapshot);

        loop {
            // We would like to do all of these receives in parallel, but sync channels
            // do not support this, so just loop through them quickly.
            match self
                .downstream
                .requests_rx
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(req) => match req {
                    Request::GetChildVersion(uuid, sender) => {
                        info!("{}: Got GetChildVersion request", self.name);
                        if let Some(v) = self.server_db.get_child_version(uuid) {
                            // We have the version locally, so just send that.
                            sender.send(Some(v)).expect("send failed");
                        } else {
                            // We do not have the version locally, so call upstream.
                            let (tx, rx) = mpsc::channel();
                            self.upstream.requests.send(Request::GetChildVersion(
                                self.server_db.get_current_version_id(),
                                tx,
                            )).expect("send failed");
                            let from_upstream = rx.recv().expect("receive failed");
                            // Cache the response locally
                            // TODO: also cache negative results here (None)
                            if let Some(v) = &from_upstream {
                                self.server_db.cache_version(uuid, v.clone());
                            }
                            sender.send(from_upstream).expect("send failed")
                        }
                    }
                    Request::GetSnapshot(sender) => {
                        info!("{}: Got GetSnapshot request", self.name);
                        // Always send a local snapshot, rather than deferring this to
                        // upstream.
                        sender.send(self.server_db.get_snapshot()).expect("send failed");
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => {},
                Err(e) => panic!("error receiving requests: {e}"),
            }

            // Update to the most recent version, if there's a notice.
            if self.upstream.notices.try_recv().is_ok() {
                // Consume all of the notices, in case they "bunch up".
                while self.upstream.notices.try_recv().is_ok() {}

                info!("{}: Intermediate updating to latest", self.name);
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

                info!("{}: Sending new-version notice", self.name);
                self.downstream.notices.broadcast(Notice::NewVersion);
            }
        }
    }

    fn get_snapshot(&mut self) -> Snapshot {
        let (tx, rx) = mpsc::channel();
        self.upstream.requests.send(Request::GetSnapshot(tx)).expect("send failed");
        rx.recv().expect("receive failed")
    }
}

