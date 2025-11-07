use log::info;
use rand::prelude::IndexedRandom;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

use crate::Downstream;
use crate::ServerDb;
use crate::Update;
use crate::Upstream;
use crate::Version;

/// Number of keys to use in the fake intake.
const FAKE_INTAKE_NUM_KEYS: usize = 2;

/// Intake is where data enters the system to be distributed downward
/// to subsidiary nodes.
pub struct FakeIntake {
    name: String,
    downstream: Downstream,
    server_db: ServerDb,
    keys: [Uuid; FAKE_INTAKE_NUM_KEYS],
}

impl FakeIntake {
    pub fn new(name: String, server_db: ServerDb) -> Self {
        Self {
            name,
            downstream: Downstream::new(),
            server_db,
            keys: (0..FAKE_INTAKE_NUM_KEYS)
                .map(|_| Uuid::new_v4())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
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
            self.downstream.serve_requests(
                |uuid| self.server_db.get_child_version(uuid),
                || self.server_db.get_snapshot(),
                Duration::from_millis(400),
            );

            // Create a new version
            let new_version_id = Uuid::new_v4();
            info!("{}: Creating version {}", self.name, new_version_id);
            let v = Version {
                version_id: new_version_id,
                updates: self.random_updates(),
            };
            self.server_db.apply_version(v);

            // Notify downstream of new version
            info!("{}: Sending new-version notice", self.name);
            self.downstream.notify();
        }
    }

    fn random_key(&mut self) -> String {
        self.keys
            .choose(&mut rand::rng())
            .cloned()
            .unwrap()
            .to_string()
    }

    fn random_value(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn random_updates(&mut self) -> Vec<Update> {
        let n = rand::random::<u8>().saturating_add(1);
        let mut updates = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let key = self.random_key();
            if rand::random::<bool>() {
                updates.push(Update::Set(key, self.random_value()))
            } else {
                updates.push(Update::Delete(key))
            }
        }
        updates
    }
}
