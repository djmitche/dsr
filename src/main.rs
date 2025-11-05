use std::{thread, time::Duration};

use dsr::*;

pub fn main() {
    pretty_env_logger::init_timed();
    let db = ServerDb::default();
    let mut intake = FakeIntake::new(db);
    let dbg1 = DebugLeaf::new(intake.make_upstream());
    let mut inter = CachingIntermediate::new(intake.make_upstream());
    let dbg2 = DebugLeaf::new(inter.make_upstream());
    let dbg3 = DebugLeaf::new(inter.make_upstream());
    intake.start();
    inter.start();
    dbg1.start();

    thread::sleep(Duration::from_secs(3));
    dbg2.start();
    dbg3.start();
    thread::sleep(Duration::from_secs(7));
}
