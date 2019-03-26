#[macro_use] extern crate failure;
#[cfg(feature = "pulse")] extern crate libpulse_sys;
extern crate x11;

#[cfg(feature = "pulse")] pub mod pulse;
pub mod x11api;

#[derive(Debug, Fail)]
pub enum MyError {
    #[fail(display = "failed to open x display")]
    XDisplay,
    #[fail(display = "failed to query for screen saver info")]
    XScreenSaver
}
