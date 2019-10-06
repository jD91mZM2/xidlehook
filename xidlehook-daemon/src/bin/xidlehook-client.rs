use std::{io::prelude::*, os::unix::net::UnixStream, time::Duration};

use structopt::{clap::arg_enum, StructOpt};

#[allow(dead_code)]
mod socket_models {
    include!("../bin_impl/unstable/socket_models.rs");
}
use self::socket_models::TimerId;

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
        index: Option<TimerId>,
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
        timer: Vec<TimerId>,
        /// Which action to cause on the selected timers
        #[structopt(long, possible_values = &OptAction::variants(), case_insensitive = true)]
        action: OptAction,
    },
    /// Query the list of timers
    Query {
        /// The timers which this operation should apply to. Leave
        /// empty for all timers.
        #[structopt(long)]
        timer: Vec<TimerId>,
    },
}

fn filter(filter: Vec<TimerId>) -> socket_models::Filter {
    if filter.is_empty() {
        socket_models::Filter::Any
    } else {
        socket_models::Filter::Selected(filter)
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
        } => socket_models::Message::Add(socket_models::Add {
            duration: Duration::from_secs(duration),
            index,
            activation,
            abortion,
            deactivation,
        }),
        Subcommands::Control { timer, action } => {
            socket_models::Message::Control(socket_models::Control {
                timer: filter(timer),
                action: match action {
                    OptAction::Enable => socket_models::Action::Enable,
                    OptAction::Disable => socket_models::Action::Disable,
                    OptAction::Trigger => socket_models::Action::Trigger,
                    OptAction::Delete => socket_models::Action::Delete,
                },
            })
        },
        Subcommands::Query { timer } => socket_models::Message::Query(socket_models::Query {
            timer: filter(timer),
        }),
    };

    let mut stream = UnixStream::connect(opt.socket)?;
    let json = serde_json::to_string(&packet)?;
    stream.write_all(json.as_bytes())?;

    Ok(())
}
