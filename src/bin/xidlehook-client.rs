#[macro_use] extern crate clap;
extern crate failure;

use clap::{App as ClapApp, Arg};
use failure::Error;
use std::{io::Write, os::unix::net::UnixStream};

fn main() -> Result<(), Error> {
    let clap_app = ClapApp::new(crate_name!())
        .author(crate_authors!())
        .version(crate_version!())
        // Flags
        .arg(
            Arg::with_name("disable")
                .help("Disable xidlehook timers.")
                .long("disable")
                .required_unless_one(&["enable", "trigger"]),
        )
        .arg(
            Arg::with_name("enable")
                .help("Enable xidlehook timers.")
                .long("enable")
                .required_unless_one(&["disable", "trigger"]),
        )
        .arg(
            Arg::with_name("trigger")
                .help("Execute the primary timer immediately.")
                .long("trigger")
                .required_unless_one(&["disable", "enable"]),
        )
        // Options
        .arg(
            Arg::with_name("socket")
                .long_help(
                    "\
                     Listen to events over a specified unix socket.\n\
                     Events are as following:\n\
                     \t0x0 - Disable xidlehook\n\
                     \t0x1 - Re-enable xidlehook\n\
                     \t0x2 - Trigger the timer immediately\n\
                     ",
                )
                .long("socket")
                .takes_value(true)
                .required(true),
        );
    let matches = clap_app.get_matches();
    // socket necessarily exists as it is a required param
    let socket = matches.value_of("socket").unwrap();
    let mut socket = UnixStream::connect(&socket)?;
    let control_byte = if matches.is_present("disable") {
        b"\x00"
    } else if matches.is_present("enable") {
        b"\x01"
    } else {
        b"\x02"
    };
    socket.write_all(control_byte)?;
    return Ok(());
}
