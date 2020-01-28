use std::{cell::Cell, time::Duration};
use xidlehook_core::{timers::CallbackTimer, Xidlehook};

const TEST_UNIT: Duration = Duration::from_millis(50);

#[test]
fn general_timer_test() {
    let triggered = Cell::new(0);

    let mut timer = Xidlehook::new(vec![
        CallbackTimer::new(TEST_UNIT * 100, || triggered.set(triggered.get() | 1)),
        CallbackTimer::new(TEST_UNIT * 010, || triggered.set(triggered.get() | 1 << 1)),
        CallbackTimer::new(TEST_UNIT * 050, || triggered.set(triggered.get() | 1 << 2)),
        CallbackTimer::new(TEST_UNIT * 200, || triggered.set(triggered.get() | 1 << 3)),
    ]);

    // Test first timer
    assert_eq!(timer.poll(TEST_UNIT * 0).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(timer.poll(TEST_UNIT * 20).unwrap(), Some(TEST_UNIT * 80));
    assert_eq!(timer.poll(TEST_UNIT * 40).unwrap(), Some(TEST_UNIT * 60));
    assert_eq!(timer.poll(TEST_UNIT * 74).unwrap(), Some(TEST_UNIT * 26));
    assert_eq!(timer.poll(TEST_UNIT * 99).unwrap(), Some(TEST_UNIT * 1));

    // Trigger first timer
    assert_eq!(triggered.get(), 0b0000);
    assert_eq!(timer.poll(TEST_UNIT * 100).unwrap(), Some(TEST_UNIT * 10));
    assert_eq!(triggered.get(), 0b0001);

    // Test second timer
    assert_eq!(timer.poll(TEST_UNIT * 103).unwrap(), Some(TEST_UNIT * 7));

    // Overshoot second timer
    assert_eq!(triggered.get(), 0b0001);
    assert_eq!(timer.poll(TEST_UNIT * 500).unwrap(), Some(TEST_UNIT * 50));
    assert_eq!(triggered.get(), 0b0011);

    // Test third timer
    assert_eq!(timer.poll(TEST_UNIT * 500).unwrap(), Some(TEST_UNIT * 50));
    assert_eq!(timer.poll(TEST_UNIT * 501).unwrap(), Some(TEST_UNIT * 49));
    assert_eq!(timer.poll(TEST_UNIT * 549).unwrap(), Some(TEST_UNIT * 1));

    // Trigger third timer
    assert_eq!(triggered.get(), 0b0011);
    assert_eq!(timer.poll(TEST_UNIT * 550).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b0111);

    // Test fourth timer
    triggered.set(0);
    assert_eq!(timer.poll(TEST_UNIT * 600).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(timer.poll(TEST_UNIT * 649).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(timer.poll(TEST_UNIT * 650).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b0000); // no change
    assert_eq!(timer.poll(TEST_UNIT * 680).unwrap(), Some(TEST_UNIT * 70));

    // Trigger fourth timer
    assert_eq!(triggered.get(), 0b0000); // no change
    assert_eq!(timer.poll(TEST_UNIT * 750).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b1000);

    // It resets
    triggered.set(0);
    assert_eq!(timer.poll(TEST_UNIT * 0).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b0000);
    assert_eq!(timer.poll(TEST_UNIT * 101).unwrap(), Some(TEST_UNIT * 10));
    assert_eq!(triggered.get(), 0b0001);
}

#[test]
fn disabled_timers() {
    let triggered = Cell::new(0);

    let mut timer = Xidlehook::new(vec![
        CallbackTimer::new(TEST_UNIT * 08, || triggered.set(triggered.get() | 1)),
        CallbackTimer::new(TEST_UNIT * 16, || triggered.set(triggered.get() | 1 << 1)),
        CallbackTimer::new(TEST_UNIT * 04, || triggered.set(triggered.get() | 1 << 2)),
        CallbackTimer::new(TEST_UNIT * 06, || triggered.set(triggered.get() | 1 << 3)),
    ]);

    // Just one good old test round first
    assert_eq!(timer.poll(TEST_UNIT * 00).unwrap(), Some(TEST_UNIT * 08));
    assert_eq!(timer.poll(TEST_UNIT * 08).unwrap(), Some(TEST_UNIT * 08)); // timer 1
    assert_eq!(timer.poll(TEST_UNIT * 24).unwrap(), Some(TEST_UNIT * 04)); // timer 2
    assert_eq!(timer.poll(TEST_UNIT * 28).unwrap(), Some(TEST_UNIT * 06)); // timer 3
    assert_eq!(timer.poll(TEST_UNIT * 34).unwrap(), Some(TEST_UNIT * 08)); // timer 4
    assert_eq!(triggered.get(), 0b1111);

    // Now disable the first timer and reset
    timer.timers_mut().unwrap()[0].disabled = true;
    triggered.set(0);

    // Make sure first timer is ignored
    assert_eq!(timer.poll(TEST_UNIT * 00).unwrap(), Some(TEST_UNIT * 16));
    assert_eq!(timer.poll(TEST_UNIT * 08).unwrap(), Some(TEST_UNIT * 08)); // ~timer 1~
    assert_eq!(triggered.get(), 0b0000);
    assert_eq!(timer.poll(TEST_UNIT * 16).unwrap(), Some(TEST_UNIT * 04)); // timer 2
    assert_eq!(timer.poll(TEST_UNIT * 20).unwrap(), Some(TEST_UNIT * 06)); // timer 3
    assert_eq!(timer.poll(TEST_UNIT * 26).unwrap(), Some(TEST_UNIT * 16)); // timer 4
    assert_eq!(triggered.get(), 0b1110);

    // Now disable a timer in the middle and reset
    timer.timers_mut().unwrap()[2].disabled = true;
    triggered.set(0);

    // Make sure first timer is ignored
    assert_eq!(timer.poll(TEST_UNIT * 00).unwrap(), Some(TEST_UNIT * 16));
    assert_eq!(timer.poll(TEST_UNIT * 08).unwrap(), Some(TEST_UNIT * 08)); // ~timer 1~
    assert_eq!(triggered.get(), 0b0000);
    assert_eq!(timer.poll(TEST_UNIT * 16).unwrap(), Some(TEST_UNIT * 06)); // timer 2
    assert_eq!(timer.poll(TEST_UNIT * 20).unwrap(), Some(TEST_UNIT * 02)); // ~timer 3~
    assert_eq!(timer.poll(TEST_UNIT * 22).unwrap(), Some(TEST_UNIT * 16)); // timer 4
    assert_eq!(triggered.get(), 0b1010);
}
