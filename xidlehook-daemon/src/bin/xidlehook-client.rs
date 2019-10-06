use std::{io::{prelude::*, BufReader, LineWriter}, os::unix::net::UnixStream, time::Duration};

use structopt::{clap::arg_enum, StructOpt};

#[allow(dead_code)]
mod socket {
    include!("../socket/models.rs");
}

arg_enum! {
    #[derive(Debug)]
    enum OptAction {
        Enable,
        Disable,
        Trigger,
        Delete,
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    /// Listen to a unix socket at this address for events.
    /// Each event is one line of JSON data.
    #[structopt(long)]
    socket: String,

    /// Specify the subcommand
    #[structopt(subcommand)]
    cmd: Subcommands,
}
#[derive(StructOpt, Debug)]
enum Subcommands {
    /// Create a new timer
    Add {
        /// The duration of the timer
        #[structopt(long)]
        duration: u64,
        /// Where to insert this timer. To insert it at the beginning,
        /// set this to 0. To insert it at the end, skip this.
        #[structopt(long)]
        index: Option<socket::TimerId>,
        /// The shell command to run on activation, *not* passed to
        /// "sh -c" (unlike the regular application)
        #[structopt(long, value_terminator = ";", allow_hyphen_values = true)]
        activation: Vec<String>,
        /// The shell command to run on abortion/cancellation (when
        /// the users stops being idle before the timer was
        /// deactivated)
        #[structopt(long, value_terminator = ";", allow_hyphen_values = true)]
        abortion: Vec<String>,
        /// The shell command to run when the next timer was activated
        /// instead. Not present in the regular application as it is
        /// not needed when you can control all the timers (my
        /// deactivation = next timer's activation basically)
        #[structopt(long, value_terminator = ";", allow_hyphen_values = true)]
        deactivation: Vec<String>,
    },
    /// A control operation
    Control {
        /// The timers which this operation should apply to. Leave
        /// empty for all timers.
        #[structopt(long)]
        timer: Vec<socket::TimerId>,
        /// Which action to cause on the selected timers
        #[structopt(long, possible_values = &OptAction::variants(), case_insensitive = true)]
        action: OptAction,
    },
    /// Query the list of timers
    Query {
        /// The timers which this operation should apply to. Leave
        /// empty for all timers.
        #[structopt(long)]
        timer: Vec<socket::TimerId>,
    },
}

fn filter(filter: Vec<socket::TimerId>) -> socket::Filter {
    if filter.is_empty() {
        socket::Filter::Any
    } else {
        socket::Filter::Selected(filter)
    }
}

fn main() -> xidlehook::Result<()> {
    let opt = Opt::from_args();
    let packet = match opt.cmd {
        Subcommands::Add {
            duration,
            index,
            activation,
            abortion,
            deactivation,
        } => socket::Message::Add(socket::Add {
            duration: Duration::from_secs(duration),
            index,
            activation,
            abortion,
            deactivation,
        }),
        Subcommands::Control { timer, action } => {
            socket::Message::Control(socket::Control {
                timer: filter(timer),
                action: match action {
                    OptAction::Enable => socket::Action::Enable,
                    OptAction::Disable => socket::Action::Disable,
                    OptAction::Trigger => socket::Action::Trigger,
                    OptAction::Delete => socket::Action::Delete,
                },
            })
        },
        Subcommands::Query { timer } => socket::Message::Query(socket::Query {
            timer: filter(timer),
        }),
    };

    let stream = UnixStream::connect(opt.socket)?;
    let mut reader = BufReader::new(&stream);
    let mut writer = LineWriter::new(&stream);

    dbg!();
    serde_json::to_writer(&mut writer, &packet)?;
    dbg!();
    writer.write_all(&[b'\n'])?;
    dbg!();
    writer.flush()?;
    dbg!();

    // TODO: This blocks forever
    let reply: socket::Reply = serde_json::from_reader(&mut reader)?;
    dbg!();

    println!("{:#?}", reply);

    Ok(())
}
