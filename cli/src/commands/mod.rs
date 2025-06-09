pub mod get;

use std::fmt;
use clap::ValueEnum;

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum ResourceType {
    Nodes,
    Pods,
    Services,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ResourceType::Nodes => "nodes",
            ResourceType::Pods => "pods",
            ResourceType::Services => "services",
        };
        write!(f, "{}", s)
    }
}
