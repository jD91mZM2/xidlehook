use std::time::Duration;
use xidlehook_core::{timers::CmdTimer, Action::*, Xidlehook};

#[test]
fn no_timers() {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut timer = Xidlehook::<CmdTimer, ()>::new(vec![]);

    assert_eq!(timer.poll(Duration::from_secs(0)).unwrap(), Forever);
}
