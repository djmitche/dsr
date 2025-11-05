#![allow(unused)]
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

// ---

#[derive(Debug, Clone)]
pub struct Version {
    version_id: Uuid,
}

// ---

#[derive(Debug)]
pub struct Snapshot {
    version_id: Uuid,
    data: HashMap<String, String>,
}

// ---

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

// ---

#[derive(Clone, Debug)]
pub enum Notice {
    /// A new version is available from upstream.
    NewVersion,
}

// ---

/// Downstream represents a link to a downstream component, allowing receiving requests and
/// sending notices.
pub struct Downstream {
    requests_rx: mpsc::Receiver<Request>,
    requests_tx: mpsc::Sender<Request>,
    notices: bus::Bus<Notice>,
}

// ---

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

// ---

/// Upstream represents a link to an upstream component, allowing sending requests and receiving
/// notices.
pub struct Upstream {
    requests: mpsc::Sender<Request>,
    notices: bus::BusReader<Notice>,
}

// ---

/// Intake is where data enters the system to be distributed downward
/// to subsidiary nodes.
pub struct FakeIntake {
    downstream: Downstream,
    server_db: ServerDb,
}

impl FakeIntake {
    pub fn new(server_db: ServerDb) -> Self {
        Self {
            downstream: Downstream::new(),
            server_db,
        }
    }

    pub fn make_upstream(&mut self) -> Upstream {
        self.downstream.make_upstream()
    }

    pub fn start(self) {
        thread::spawn(move || self.run());
    }

    fn run(mut self) {
        loop {
            // Serve some requests
            loop {
                match self
                    .downstream
                    .requests_rx
                    .recv_timeout(Duration::from_millis(400))
                {
                    Ok(req) => match req {
                        Request::GetChildVersion(uuid, sender) => {
                            sender.send(self.server_db.get_child_version(uuid));
                        }
                        Request::GetSnapshot(sender) => {
                            sender.send(self.server_db.get_snapshot());
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(e) => panic!("error receiving requests: {e}"),
                }
            }

            // Create a new version
            let new_version_id = Uuid::new_v4();
            info!("Creating version {}", new_version_id);
            let v = Version {
                version_id: new_version_id,
            };
            self.server_db.add_version(v);

            // Notify downstream of new version
            info!("Sending new-version notice");
            self.downstream.notices.broadcast(Notice::NewVersion);
        }
    }
}

// ---

pub struct CachingIntermediate {
    upstream: Upstream,
    downstream: Downstream,
    server_db: ServerDb,
}

impl CachingIntermediate {
    pub fn new(upstream: Upstream) -> Self {
        Self {
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
        info!("Starting at snapshot version {}", snapshot.version_id);
        self.server_db.apply_snapshot(snapshot);

        loop {
            // We would like to do all of these receives in parallel, but sync channels
            // do not support this, so just loop through them quickly.
            info!("Intermediate handling requests");
            match self
                .downstream
                .requests_rx
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(req) => match req {
                    Request::GetChildVersion(uuid, sender) => {
                        if let Some(v) = self.server_db.get_child_version(uuid) {
                            // We have the version locally, so just send that.
                            sender.send(Some(v)).expect("send failed");
                        } else {
                            // We do not have the version locally, so call upstream.
                            let (tx, rx) = mpsc::channel();
                            self.upstream.requests.send(Request::GetChildVersion(
                                self.server_db.get_current_version_id(),
                                tx,
                            ));
                            let from_upstream = rx.recv().expect("receive failed");
                            // Cache the response locally
                            // TODO: also cache negative results here (None)
                            if let Some(v) = &from_upstream {
                                self.server_db.cache_version(uuid, v.clone());
                            }
                            sender
                                .send(from_upstream)
                                .expect("send failed")
                        }
                    }
                    Request::GetSnapshot(sender) => {
                        // Always send a local snapshot, rather than deferring this to
                        // upstream.
                        sender.send(self.server_db.get_snapshot());
                    }
                },
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(e) => panic!("error receiving requests: {e}"),
            }

            // Update to the most recent version, if there's a notice.
            if self.upstream.notices.try_recv().is_ok() {
                info!("Intermediate updating to latest");
                loop {
                    let (tx, rx) = mpsc::channel();
                    self.upstream.requests.send(Request::GetChildVersion(
                        self.server_db.get_current_version_id(),
                        tx,
                    ));
                    if let Some(v) = rx.recv().expect("receive failed") {
                        info!("Got version {}", v.version_id);
                        self.server_db.add_version(v);
                    } else {
                        break;
                    }
                }
            }
        }

        for notice in self.upstream.notices.iter() {
            info!("Got notice {notice:?}");
        }
    }

    fn get_snapshot(&mut self) -> Snapshot {
        let (tx, rx) = mpsc::channel();
        self.upstream.requests.send(Request::GetSnapshot(tx));
        rx.recv().expect("receive failed")
    }
}

// ---

pub struct DebugLeaf {
    upstream: Upstream,
    server_db: ServerDb,
}

impl DebugLeaf {
    pub fn new(upstream: Upstream) -> Self {
        Self {
            upstream,
            server_db: ServerDb::default(),
        }
    }

    pub fn start(self) {
        thread::spawn(move || self.run());
    }

    fn run(mut self) {
        let snapshot = self.get_snapshot();
        info!("Starting at snapshot version {}", snapshot.version_id);
        self.server_db.apply_snapshot(snapshot);
        loop {
            // Update to the most recent version.
            loop {
                let (tx, rx) = mpsc::channel();
                self.upstream.requests.send(Request::GetChildVersion(
                    self.server_db.get_current_version_id(),
                    tx,
                ));
                if let Some(v) = rx.recv().expect("receive failed") {
                    info!("Got version {}", v.version_id);
                    self.server_db.add_version(v);
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
        self.upstream.requests.send(Request::GetSnapshot(tx));
        rx.recv().expect("receive failed")
    }
}

// ---

/// ServerDB is a handle to the server data, and can be cloned and used
/// concurrently from multiple threads.
#[derive(Clone, Default)]
pub struct ServerDb {
    /// Data as of the latest version
    inner: Arc<Mutex<ServerDbInner>>,
}

#[derive(Default)]
struct ServerDbInner {
    /// Current version, the latest in the sequence of versions.
    current_version_id: Uuid,
    /// Versions, indexed by parent version ID
    versions: HashMap<Uuid, Version>,
    /// Data as of the latest version
    data: HashMap<String, String>,
}

impl ServerDb {
    fn get_current_version_id(&self) -> Uuid {
        self.inner
            .lock()
            .expect("lock failed")
            .get_current_version_id()
    }

    fn add_version(&self, v: Version) {
        self.inner.lock().expect("lock failed").add_version(v);
    }

    fn cache_version(&self, parent_version_id: Uuid, v: Version) {
        self.inner.lock().expect("lock failed").cache_version(parent_version_id, v);
    }

    fn get_child_version(&self, uuid: Uuid) -> Option<Version> {
        self.inner
            .lock()
            .expect("lock failed")
            .get_child_version(uuid)
    }

    fn get_snapshot(&mut self) -> Snapshot {
        self.inner.lock().expect("lock failed").get_snapshot()
    }

    fn apply_snapshot(&self, snapshot: Snapshot) {
        self.inner
            .lock()
            .expect("lock failed")
            .apply_snapshot(snapshot)
    }
}

impl ServerDbInner {
    fn get_current_version_id(&mut self) -> Uuid {
        self.current_version_id
    }

    fn add_version(&mut self, v: Version) {
        let new_version_id = v.version_id;
        self.versions.insert(self.current_version_id, v);
        self.current_version_id = new_version_id;
    }

    fn cache_version(&mut self, parent_version_id: Uuid, v: Version) {
        self.versions.insert(parent_version_id, v);
    }

    fn get_child_version(&mut self, parent_version_id: Uuid) -> Option<Version> {
        self.versions.get(&parent_version_id).cloned()
    }

    fn get_snapshot(&mut self) -> Snapshot {
        Snapshot {
            version_id: self.current_version_id,
            data: self.data.clone(),
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.current_version_id = snapshot.version_id;
        self.data = snapshot.data;
    }
}
