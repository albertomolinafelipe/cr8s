use std::sync::RwLock;

use shared::Node;

#[derive(Debug)]
pub struct ClusterState {
    nodes: RwLock<Vec<Node>>,
}

impl ClusterState {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(Vec::new()),
        }
    }

    pub fn add_node(&self, node: Node) {
        let mut nodes = self.nodes.write().unwrap();
        nodes.push(node);
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        let nodes = self.nodes.read().unwrap();
        nodes.clone()
    }
}

