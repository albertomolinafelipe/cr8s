pub mod create;
pub mod get;

use clap::ValueEnum;
use std::fmt;

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum ResourceType {
    Nodes,
    Pods,
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
