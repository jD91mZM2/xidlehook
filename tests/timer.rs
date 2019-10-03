use std::{cell::Cell, time::Duration};
use xidlehook::{Result, Timer, Xidlehook};

const TEST_UNIT: Duration = Duration::from_millis(50);

struct TestTimer<'a> {
    time: Duration,
    f: Box<dyn FnMut() + 'a>,
}
impl<'a> TestTimer<'a> {
    pub fn new<F>(time: u32, f: F) -> Self
    where
        F: FnMut() + 'a,
    {
        Self {
            time: TEST_UNIT * time,
            f: Box::new(f),
        }
    }
}
impl<'a> Timer for TestTimer<'a> {
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>> {
        Ok(self
            .time
            .checked_sub(idle_time)
            .filter(|&d| d != Duration::default()))
    }
    fn activate(&mut self) -> Result<()> {
        (self.f)();
        Ok(())
    }
}

#[test]
fn general_timer_test() {
    let triggered = Cell::new(0);

    let mut timer = Xidlehook::new(vec![
        TestTimer::new(100, || triggered.set(triggered.get() | 1)),
        TestTimer::new(10, || triggered.set(triggered.get() | 1 << 1)),
        TestTimer::new(50, || triggered.set(triggered.get() | 1 << 2)),
        TestTimer::new(200, || triggered.set(triggered.get() | 1 << 3)),
    ]);

    // Test first timer
    assert_eq!(timer.poll(TEST_UNIT * 20).unwrap(), Some(TEST_UNIT * 80));
    assert_eq!(timer.poll(TEST_UNIT * 40).unwrap(), Some(TEST_UNIT * 60));
    assert_eq!(timer.poll(TEST_UNIT * 74).unwrap(), Some(TEST_UNIT * 26));
    assert_eq!(timer.poll(TEST_UNIT * 99).unwrap(), Some(TEST_UNIT * 1));

    // Trigger first timer
    assert_eq!(triggered.get(), 0);
    assert_eq!(timer.poll(TEST_UNIT * 100).unwrap(), Some(TEST_UNIT * 10));
    assert_eq!(triggered.get(), 1);

    // Test second timer
    assert_eq!(timer.poll(TEST_UNIT * 103).unwrap(), Some(TEST_UNIT * 7));

    // Overshoot second timer
    assert_eq!(triggered.get(), 1);
    assert_eq!(timer.poll(TEST_UNIT * 500).unwrap(), Some(TEST_UNIT * 50));
    assert_eq!(triggered.get(), 0b11);

    // Test third timer
    assert_eq!(timer.poll(TEST_UNIT * 500).unwrap(), Some(TEST_UNIT * 50));
    assert_eq!(timer.poll(TEST_UNIT * 501).unwrap(), Some(TEST_UNIT * 49));
    assert_eq!(timer.poll(TEST_UNIT * 549).unwrap(), Some(TEST_UNIT * 1));

    // Trigger third timer
    assert_eq!(triggered.get(), 0b11);
    assert_eq!(timer.poll(TEST_UNIT * 550).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b111);

    // Test fourth timer
    assert_eq!(timer.poll(TEST_UNIT * 600).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(timer.poll(TEST_UNIT * 649).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(timer.poll(TEST_UNIT * 650).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b111); // no change
    assert_eq!(timer.poll(TEST_UNIT * 680).unwrap(), Some(TEST_UNIT * 70));

    // Trigger fourth timer
    assert_eq!(triggered.get(), 0b111);
    assert_eq!(timer.poll(TEST_UNIT * 750).unwrap(), Some(TEST_UNIT * 100));
    assert_eq!(triggered.get(), 0b1111);
}
