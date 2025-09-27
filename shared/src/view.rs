//! Shared view logic for formatting models (Node, PodObject) into table displays.
//! Includes `Tabled` implementations and status formatting helpers.

use std::borrow::Cow;

use chrono::Utc;
use tabled::Tabled;

use crate::models::{
    node::{Node, NodeStatus},
    pod::{Pod, PodPhase},
    replicaset::ReplicaSet,
};

// --- Display impls for status enums ---

/// String representation of `NodeStatus` for table output.
impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Ready => write!(f, "Ready"),
            NodeStatus::Running => write!(f, "Running"),
            NodeStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

/// String representation of `PodPhase` for table output.
impl std::fmt::Display for PodPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PodPhase::Pending => write!(f, "Pending"),
            PodPhase::Running => write!(f, "Running"),
            PodPhase::Failed => write!(f, "Failed"),
            PodPhase::Succeeded => write!(f, "Succeeded"),
            PodPhase::Unknown => write!(f, "Unknown"),
        }
    }
}

// --- Node ---

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

// --- Pod ---

impl Tabled for Pod {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        // Only count containers considered "ready" by allowed status strings.
        let good_statuses = [
            // "empty",
            "created",
            "running",
            //"paused",
            //"restarting",
            //"removing",
            //"exited",
            //"dead",
        ];

        let total_containers = self.spec.containers.len();

        let ready_count = if self.status.last_update.is_none() {
            0
        } else {
            self.status
                .container_status
                .iter()
                .filter(|(_, status)| good_statuses.contains(&status.as_str()))
                .count()
        };

        vec![
            Cow::Owned(self.metadata.name.clone()),
            Cow::Owned(format!("{}/{}", ready_count, total_containers)),
            Cow::Owned(self.status.phase.to_string()),
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

// --- ReplicaSet ---

impl Tabled for ReplicaSet {
    const LENGTH: usize = 5;

    fn fields(&self) -> Vec<Cow<'_, str>> {
        vec![
            Cow::Owned(self.metadata.name.clone()),
            Cow::Owned(self.spec.replicas.to_string()),
            Cow::Owned(self.status.ready_replicas.to_string()),
            Cow::Owned(self.metadata.generation.to_string()),
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
            Cow::Borrowed("REPLICAS"),
            Cow::Borrowed("READY"),
            Cow::Borrowed("GENERATION"),
            Cow::Borrowed("AGE"),
        ]
    }
}

// --- Utility functions ---

/// Converts a `Duration` into a human-readable age string like `5m ago`, `2h ago`, etc.
fn human_duration(dur: std::time::Duration) -> String {
    let secs = dur.as_secs();
    match secs {
        0..=59 => format!("{}s ago", secs),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}
