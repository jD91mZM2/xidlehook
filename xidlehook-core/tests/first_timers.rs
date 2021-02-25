use std::{cell::Cell, time::Duration};
use xidlehook_core::{timers::CallbackTimer, Action::*, Xidlehook};

const TEST_UNIT: Duration = Duration::from_millis(50);

#[test]
fn first_timer_test() {
    let triggered = Cell::new(0);

    let mut timer = Xidlehook::new(vec![
        CallbackTimer::new(TEST_UNIT * 100, || triggered.set(triggered.get() | 1)),
        CallbackTimer::new(TEST_UNIT * 010, || triggered.set(triggered.get() | 1 << 1)),
        CallbackTimer::new(TEST_UNIT * 050, || triggered.set(triggered.get() | 1 << 2)),
        CallbackTimer::new(TEST_UNIT * 200, || triggered.set(triggered.get() | 1 << 3)),
    ]);

    // Trigger all timers up to the last one.
    assert_eq!(timer.poll(TEST_UNIT * 100).unwrap(), Sleep(TEST_UNIT * 010));
    assert_eq!(timer.poll(TEST_UNIT * 110).unwrap(), Sleep(TEST_UNIT * 050));

    // The sleep would now be 200, except the first timer
    // could be reactivated by activity so sleep is limited to 100.
    assert_eq!(timer.poll(TEST_UNIT * 160).unwrap(), Sleep(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b0111);

    timer.timers_mut().unwrap()[0].disabled = true;

    triggered.set(0);

    // Trigger all timers up to the last one. The sleep is always limited to 10 because the first
    // enabled timer can be accessed at any time and thus 10 sleep is the minimum.
    assert_eq!(timer.poll(TEST_UNIT * 000).unwrap(), Sleep(TEST_UNIT * 010));
    assert_eq!(timer.poll(TEST_UNIT * 010).unwrap(), Sleep(TEST_UNIT * 010));
    assert_eq!(timer.poll(TEST_UNIT * 060).unwrap(), Sleep(TEST_UNIT * 010));
    assert_eq!(timer.poll(TEST_UNIT * 260).unwrap(), Sleep(TEST_UNIT * 010));
    assert_eq!(triggered.get(), 0b1110);
}
