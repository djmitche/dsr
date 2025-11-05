use std::{thread, time::Duration};
use log::warn;

use dsr::*;

pub fn main() {
    pretty_env_logger::init_timed();
    let db = ServerDb::default();
    let mut intake = FakeIntake::new("intake".into(), db);
    let dbg1 = DebugLeaf::new("dbg1".into(), intake.make_upstream());
    let mut inter = CachingIntermediate::new("inter".into(), intake.make_upstream());
    let dbg2 = DebugLeaf::new("dbg2".into(), inter.make_upstream());
    let dbg3 = DebugLeaf::new("dbg3".into(), inter.make_upstream());
    intake.start();
    inter.start();
    dbg1.start();

    thread::sleep(Duration::from_secs(3));
    warn!("starting 2, 3");
    dbg2.start();
    dbg3.start();
    thread::sleep(Duration::from_secs(7));
}
