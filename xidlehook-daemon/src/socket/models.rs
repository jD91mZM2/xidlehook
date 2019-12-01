use std::time::Duration;

use serde::{Deserialize, Serialize};

pub type TimerId = u16;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Filter {
    All,
    Selected(Vec<TimerId>),
    One(TimerId),
}
impl Default for Filter {
    fn default() -> Self {
        Self::All
    }
}
impl Filter {
    pub fn iter(&'_ self, len: TimerId) -> Box<dyn Iterator<Item = TimerId> + '_> {
        match *self {
            Self::All => Box::new(0..len),
            Self::Selected(ref ids) => Box::new(ids.iter().copied()),
            Self::One(id) => Box::new(std::iter::once(id)),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Disable,
    Enable,
    Trigger,
    Delete,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Add {
    pub index: Option<TimerId>,
    pub time: Duration,
    pub activation: Vec<String>,
    pub abortion: Vec<String>,
    pub deactivation: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Control {
    #[serde(default)]
    pub timer: Filter,
    pub action: Action,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Query {
    #[serde(default)]
    pub timer: Filter,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Message {
    Add(Add),
    Control(Control),
    Query(Query),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct QueryResult {
    pub timer: TimerId,
    pub time: Duration,
    pub activation: Vec<String>,
    pub abortion: Vec<String>,
    pub deactivation: Vec<String>,
    pub disabled: bool,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Reply {
    Empty,
    Error(String),
    QueryResult(Vec<QueryResult>),
}
