use std::borrow::Cow;

use chrono::Utc;
use tabled::Tabled;

use crate::models::{Node, NodeStatus, PodObject, PodStatus};

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Ready => write!(f, "Ready"),
            NodeStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

impl std::fmt::Display for PodStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PodStatus::Pending => write!(f, "Pending"),
            PodStatus::Running => write!(f, "Running"),
            PodStatus::Failed => write!(f, "Failed"),
            PodStatus::Succeeded => write!(f, "Succeeded"),
            PodStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

impl Tabled for Node {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        vec![
            Cow::Owned(self.name.clone()),
            Cow::Owned(self.status.to_string()),
            Cow::Owned(self.addr.clone()),
            Cow::Owned(human_duration(
                Utc::now()
                    .signed_duration_since(self.started_at)
                    .to_std()
                    .unwrap_or_default(),
            )),
        ]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec![
            Cow::Borrowed("NAME"),
            Cow::Borrowed("STATUS"),
            Cow::Borrowed("ADDRESS"),
            Cow::Borrowed("AGE"),
        ]
    }
}

impl Tabled for PodObject {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        vec![
            Cow::Owned(self.metadata.user.name.clone()),
            Cow::Borrowed("1/1"),
            Cow::Owned(self.pod_status.to_string()),
            Cow::Borrowed("0"),
            Cow::Owned(human_duration(
                Utc::now()
                    .signed_duration_since(self.metadata.created_at)
                    .to_std()
                    .unwrap_or_default(),
            )),
        ]
    }

    fn headers() -> Vec<Cow<'static, str>> {
        vec![
            Cow::Borrowed("NAME"),
            Cow::Borrowed("READY"),
            Cow::Borrowed("STATUS"),
            Cow::Borrowed("RESTARTS"),
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
