use std::{
    io::{prelude::*, BufReader, LineWriter},
    os::unix::net::UnixStream,
    time::Duration,
};

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
        /// The desired idle time, in seconds, which the timer will go
        /// off after
        #[structopt(long)]
        time: u64,
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
        socket::Filter::All
    } else {
        socket::Filter::Selected(filter)
    }
}

fn main() -> xidlehook_core::Result<()> {
    let opt = Opt::from_args();
    let packet = match opt.cmd {
        Subcommands::Add {
            time,
            index,
            activation,
            abortion,
            deactivation,
        } => socket::Message::Add(socket::Add {
            time: Duration::from_secs(time),
            index,
            activation,
            abortion,
            deactivation,
        }),
        Subcommands::Control { timer, action } => socket::Message::Control(socket::Control {
            timer: filter(timer),
            action: match action {
                OptAction::Enable => socket::Action::Enable,
                OptAction::Disable => socket::Action::Disable,
                OptAction::Trigger => socket::Action::Trigger,
                OptAction::Delete => socket::Action::Delete,
            },
        }),
        Subcommands::Query { timer } => socket::Message::Query(socket::Query {
            timer: filter(timer),
        }),
    };

    let stream = UnixStream::connect(opt.socket)?;
    let reader = BufReader::new(&stream);
    let mut writer = LineWriter::new(&stream);

    serde_json::to_writer(&mut writer, &packet)?;
    writer.write_all(&[b'\n'])?;
    writer.flush()?;

    if let Some(line) = reader.lines().next() {
        let reply: socket::Reply = serde_json::from_str(&line?)?;
        println!("{:#?}", reply);
    }

    Ok(())
}
