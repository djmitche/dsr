use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

use crate::Snapshot;
use crate::Version;
use crate::Update;

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
    pub fn get_current_version_id(&self) -> Uuid {
        self.inner
            .lock()
            .expect("lock failed")
            .get_current_version_id()
    }

    pub fn apply_version(&self, v: Version) {
        self.inner.lock().expect("lock failed").apply_version(v);
    }

    pub fn cache_version(&self, parent_version_id: Uuid, v: Version) {
        self.inner
            .lock()
            .expect("lock failed")
            .cache_version(parent_version_id, v);
    }

    pub fn get_child_version(&self, uuid: Uuid) -> Option<Version> {
        self.inner
            .lock()
            .expect("lock failed")
            .get_child_version(uuid)
    }

    pub fn get_snapshot(&self) -> Snapshot {
        self.inner.lock().expect("lock failed").get_snapshot()
    }

    pub fn apply_snapshot(&self, snapshot: Snapshot) {
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

    fn apply_version(&mut self, mut v: Version) {
        for upd in v.updates.drain(..) {
            self.apply_update(upd);
        }
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

    fn apply_update(&mut self, upd: Update) {
        match upd {
            Update::Set(k, v) => {
                self.data.insert(k, v);
            }
            Update::Delete(k) => {
                self.data.remove(&k);
            }
        }
    }
}
