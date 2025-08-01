pub mod create;
pub mod delete;
pub mod get;

use clap::ValueEnum;
use std::fmt;

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum ResourceType {
    Nodes,
    Pods,
}

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum ResourceKind {
    Pod,
    Deployment,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ResourceType::Nodes => "nodes",
            ResourceType::Pods => "pods",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ResourceKind::Pod => "pod",
            ResourceKind::Deployment => "deployment",
        };
        write!(f, "{}", s)
    }
}
