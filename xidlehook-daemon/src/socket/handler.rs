use super::models::*;
use crate::{timer::CmdTimer, App};

use xidlehook::Progress;

impl App {
    pub fn handle_socket(&mut self, msg: Message) -> xidlehook::Result<Option<Reply>> {
        match msg {
            Message::Add(add) => {
                let timers = self.xidlehook.timers_mut()?;

                let index = add.index.map(usize::from).unwrap_or_else(|| timers.len());
                if index > timers.len() {
                    return Ok(Some(Reply::Error(String::from("index > length"))));
                }
                timers.insert(
                    index,
                    CmdTimer::from_parts(add.time, add.activation, add.abortion, add.deactivation),
                );

                Ok(Some(Reply::Empty))
            },
            Message::Control(control) => {
                let timers = self.xidlehook.timers();

                let mut removed = 0;
                for id in control.timer.iter(timers.len() as TimerId) {
                    let timers = self.xidlehook.timers_mut()?;

                    let id = usize::from(id - removed);
                    if id >= timers.len() {
                        continue;
                    }

                    match control.action {
                        Action::Disable => timers[id].set_disabled(true),
                        Action::Enable => timers[id].set_disabled(false),
                        Action::Trigger => {
                            if self.xidlehook.trigger(id, self.xcb.get_idle()?, true)?
                                == Progress::Stop
                            {
                                return Ok(None);
                            }
                        },
                        Action::Delete => {
                            // Probably want to use `retain` to optimize this...
                            timers.remove(id);
                            removed += 1;
                        },
                    }
                }

                Ok(Some(Reply::Empty))
            },
            Message::Query(query) => {
                let timers = self.xidlehook.timers();
                let mut output = Vec::new();

                for id in query.timer.iter(timers.len() as TimerId) {
                    let timer = match timers.get(usize::from(id)) {
                        Some(timer) => timer,
                        None => continue,
                    };
                    output.push(QueryResult {
                        timer: id,
                        activation: timer.activation().to_vec(),
                        abortion: timer.abortion().to_vec(),
                        deactivation: timer.deactivation().to_vec(),
                        disabled: timer.get_disabled(),
                    });
                }

                Ok(Some(Reply::QueryResult(output)))
            },
        }
    }
}
