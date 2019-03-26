#[macro_use] extern crate clap;
extern crate failure;

use clap::Arg;
use failure::Error;
use std::{io::Write, os::unix::net::UnixStream};

fn main() -> Result<(), Error> {
    let matches = app_from_crate!()
        // Flags
        .arg(
            Arg::with_name("disable")
                .help("Disable xidlehook timers")
                .long("disable")
                .required_unless_one(&["enable", "trigger"]),
        )
        .arg(
            Arg::with_name("enable")
                .help("Enable xidlehook timers")
                .long("enable")
                .required_unless_one(&["disable", "trigger"]),
        )
        .arg(
            Arg::with_name("trigger")
                .help("Execute the primary timer immediately")
                .long("trigger")
                .required_unless_one(&["disable", "enable"]),
        )
        // Options
        .arg(
            Arg::with_name("socket")
                .help("Specify which socket the client should communicate over")
                .long("socket")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let socket = matches.value_of("socket").unwrap();
    let mut socket = UnixStream::connect(&socket)?;

    let control_byte = if matches.is_present("disable") {
        0
    } else if matches.is_present("enable") {
        1
    } else if matches.is_present("trigger") {
        2
    } else {
        unreachable!("One of --enable, --disable or --trigger should have been set at this point");
    };
    socket.write_all(&[control_byte])?;

    return Ok(());
}
