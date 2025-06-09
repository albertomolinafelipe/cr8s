use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tabled::Tabled;
use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Node {
    pub name: String,
    pub status: NodeStatus,
    pub address: String,
    pub port: u16,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum NodeStatus {
    Ready,
    Stopped,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Ready => write!(f, "Ready"),
            NodeStatus::Stopped => write!(f, "Stopped"),
        }
    }
}


impl Tabled for Node {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        vec![
            Cow::Owned(self.name.clone()),
            Cow::Owned(self.status.to_string()),
            Cow::Owned(self.address.clone()),
            Cow::Owned(self.port.to_string()),
            Cow::Owned(human_duration(Utc::now().signed_duration_since(self.started_at).to_std().unwrap_or_default())),
        ]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec![
            Cow::Borrowed("NAME"),
            Cow::Borrowed("STATUS"),
            Cow::Borrowed("ADDRESS"),
            Cow::Borrowed("PORT"),
            Cow::Borrowed("AGE"),
        ]
    }
}


fn human_duration(dur: std::time::Duration) -> String {
    let secs = dur.as_secs();
    match secs {
        0..=59 => format!("{}s ago", secs),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}
