use super::models::*;
use crate::{timers::CmdTimer, App};

use std::convert::TryInto;

use xidlehook_core::Progress;

impl App {
    pub fn handle_socket(&mut self, msg: Message) -> xidlehook_core::Result<Option<Reply>> {
        match msg {
            Message::Add(add) => {
                let timers = self.xidlehook.timers_mut()?;

                let index = add.index.map_or_else(|| timers.len(), usize::from);
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
                let len = self.xidlehook.timers().len();

                let mut removed = 0;
                for id in control.timer.iter(
                    len.try_into()
                        .expect("xidlehook does not yet handle this many timers"),
                ) {
                    let timers = self.xidlehook.timers_mut()?;

                    let id = match id.checked_sub(removed) {
                        Some(res) => usize::from(res),
                        None => continue,
                    };
                    if id >= timers.len() {
                        continue;
                    }

                    match control.action {
                        Action::Disable => {
                            timers[id].set_disabled(true);
                        },
                        Action::Enable => {
                            timers[id].set_disabled(false);
                        },
                        Action::Trigger => {
                            if self.xidlehook.trigger(id, self.xcb.get_idle()?, true)?
                                == Progress::Stop
                            {
                                return Ok(None);
                            }
                        },
                        Action::Delete => {
                            // TODO: Probably want to use `retain` to optimize this...
                            timers.remove(id);

                            // Working with this large indices pointing to an allocated object... I
                            // think we're fine
                            removed += 1;
                        },
                    }
                }

                Ok(Some(Reply::Empty))
            },
            Message::Query(query) => {
                let timers = self.xidlehook.timers();
                let mut output = Vec::new();

                for id in query.timer.iter(
                    timers
                        .len()
                        .try_into()
                        .expect("xidlehook does not yet handle this many timers"),
                ) {
                    let timer = match timers.get(usize::from(id)) {
                        Some(timer) => timer,
                        None => continue,
                    };
                    output.push(QueryResult {
                        timer: id,
                        time: timer.get_time(),
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
