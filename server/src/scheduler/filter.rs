use shared::models::pod::Pod;

use super::{scorer::Score, state::State};

pub enum FilterOptions {
    Basic,
    // NodeSelector
}

impl FilterOptions {
    pub fn filter(&self, state: &State, pod: &Pod, candidates: &mut Vec<(String, Score)>) {
        match self {
            FilterOptions::Basic => {
                let Some(pod_res) = state.pod_resources.get(&pod.metadata.id).map(|r| r.clone())
                else {
                    tracing::warn!(pod_name=%pod.metadata.name, "Pod has no simulated resources");
                    return;
                };

                for entry in state.nodes.iter() {
                    let node_name = entry.key();
                    if let Some(node_res) = state.node_resources.get(node_name) {
                        if node_res.cpu >= pod_res.cpu && node_res.mem >= pod_res.mem {
                            candidates.push((node_name.clone(), 0.0));
                        }
                    }
                }
            }
        }
    }
}
