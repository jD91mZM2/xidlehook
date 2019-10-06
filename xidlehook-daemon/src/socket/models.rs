use std::time::Duration;

use serde::{Deserialize, Serialize};

pub type TimerId = u16;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Filter {
    Any,
    Selected(Vec<TimerId>),
    One(TimerId),
}
impl Default for Filter {
    fn default() -> Self {
        Self::Any
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
    pub duration: Duration,
    pub index: Option<TimerId>,
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
    pub activation: Vec<String>,
    pub abortion: Vec<String>,
    pub deactivation: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Reply {
    Empty,
    QueryResult(Vec<QueryResult>),
}
