use std::{process::Command, time::Duration};

use xidlehook_core::{timers::CmdTimer as Inner, Result, Timer};

pub struct CmdTimer {
    inner: Inner,

    activation: Option<Vec<String>>,
    abortion: Option<Vec<String>>,
    deactivation: Option<Vec<String>>,
}
impl CmdTimer {
    pub fn from_parts(
        time: Duration,
        activation: Vec<String>,
        abortion: Vec<String>,
        deactivation: Vec<String>,
    ) -> Self {
        let mut me = Self {
            inner: Inner {
                time,
                ..Inner::default()
            },
            activation: Some(activation).filter(|v| !v.is_empty()),
            abortion: Some(abortion).filter(|v| !v.is_empty()),
            deactivation: Some(deactivation).filter(|v| !v.is_empty()),
        };
        me.sync();
        me
    }

    pub fn from_shell(
        time: Duration,
        activation: String,
        abortion: String,
        deactivation: String,
    ) -> Self {
        let mut me = Self {
            inner: Inner {
                time,
                ..Inner::default()
            },
            activation: Some(activation)
                .filter(|s| !s.is_empty())
                .map(|s| vec!["/bin/sh".into(), "-c".into(), s]),
            abortion: Some(abortion)
                .filter(|s| !s.is_empty())
                .map(|s| vec!["/bin/sh".into(), "-c".into(), s]),
            deactivation: Some(deactivation)
                .filter(|s| !s.is_empty())
                .map(|s| vec!["/bin/sh".into(), "-c".into(), s]),
        };
        me.sync();
        me
    }

    pub fn set_disabled(&mut self, val: bool) {
        self.inner.disabled = val;
    }
    pub fn get_disabled(&self) -> bool {
        self.inner.disabled
    }

    // There's no reason to not have `set_time` here as well, it just
    // isn't available in the API yet.

    pub fn get_time(&self) -> Duration {
        self.inner.time
    }

    pub fn activation(&self) -> &[String] {
        self.activation.as_ref().map_or(&[], |v| &**v)
    }
    pub fn abortion(&self) -> &[String] {
        self.abortion.as_ref().map_or(&[], |v| &**v)
    }
    pub fn deactivation(&self) -> &[String] {
        self.deactivation.as_ref().map_or(&[], |v| &**v)
    }

    /// Propagate my fields to the inner timer
    fn sync(&mut self) {
        self.inner.activation = self
            .activation
            .as_ref()
            .map(|parts| (parts, Command::new(&parts[0])))
            .map(|(parts, mut cmd)| {
                cmd.args(&parts[1..]);
                cmd
            });
        self.inner.abortion = self
            .abortion
            .as_ref()
            .map(|parts| (parts, Command::new(&parts[0])))
            .map(|(parts, mut cmd)| {
                cmd.args(&parts[1..]);
                cmd
            });
        self.inner.deactivation = self
            .deactivation
            .as_ref()
            .map(|parts| (parts, Command::new(&parts[0])))
            .map(|(parts, mut cmd)| {
                cmd.args(&parts[1..]);
                cmd
            });
    }
}
impl Timer for CmdTimer {
    fn time_left(&mut self, idle_time: Duration) -> Result<Option<Duration>> {
        self.inner.time_left(idle_time)
    }
    fn abort_urgency(&self) -> Option<Duration> {
        self.inner.abort_urgency()
    }
    fn activate(&mut self) -> Result<()> {
        self.inner.activate()
    }
    fn abort(&mut self) -> Result<()> {
        self.inner.abort()
    }
    fn deactivate(&mut self) -> Result<()> {
        self.inner.deactivate()
    }
    fn disabled(&mut self) -> bool {
        self.inner.disabled()
    }
}
