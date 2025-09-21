use reqwest::Client;
use serde_json::Value;
use shared::{
    api::{PodField, PodPatch},
    models::pod::Pod,
};

use super::{
    filter::FilterOptions,
    scorer::{Score, Scorer},
    state::State,
};

/// Scheduling flow for a single pod: filters candidate nodes,
/// scores them, and binds the pod if a node is chosen
pub struct SchedulerFlow {
    state: State,
    pod: Pod,
    candidates: Vec<(String, f64)>,
    pub chosen: Option<String>,
    pub accepted: bool,
    filter_option: FilterOptions,
    scorer: Scorer,
}

impl SchedulerFlow {
    pub fn new(
        state: &State,
        pod: Pod,
        filter_option: Option<FilterOptions>,
        scorer: Option<Scorer>,
    ) -> Self {
        Self {
            state: state.clone(),
            pod,
            candidates: Vec::new(),
            chosen: None,
            accepted: false,
            filter_option: filter_option.unwrap_or(FilterOptions::Basic),
            scorer: scorer.unwrap_or(Scorer::Basic),
        }
    }

    pub async fn execute(self) -> Self {
        self.filter().score().bind().await
    }

    /// Apply the filter to generate an initial set of candidate nodes.
    fn filter(mut self) -> Self {
        self.filter_option
            .filter(&self.state, &self.pod, &mut self.candidates);
        self
    }

    /// Score candidate nodes and pick the best one (if any).
    fn score(mut self) -> Self {
        if self.candidates.is_empty() {
            return self;
        }

        let Some(pod_res) = self
            .state
            .pod_resources
            .get(&self.pod.metadata.id)
            .map(|r| r.clone())
        else {
            tracing::warn!(pod_name=%self.pod.metadata.name, "Pod has no simulated resources");
            return self;
        };

        let mut best: Option<(String, Score)> = None;

        for (node_name, score) in self.candidates.iter_mut() {
            if let Some(node_res) = self.state.node_resources.get(node_name) {
                let free_cpu = node_res.cpu.saturating_sub(pod_res.cpu);
                let free_mem = node_res.mem.saturating_sub(pod_res.mem);

                let pod_count = self
                    .state
                    .pod_map
                    .get(node_name)
                    .map(|set| set.len())
                    .unwrap_or(0);

                *score = self.scorer.score(pod_count, free_cpu, free_mem);

                match &best {
                    None => best = Some((node_name.clone(), *score)),
                    Some((_, best_score)) if *score > *best_score => {
                        best = Some((node_name.clone(), *score))
                    }
                    _ => {}
                }
            }
        }

        if let Some((node, _)) = best {
            self.chosen = Some(node);
        }
        self
    }

    /// Bind the pod to the chosen node by patching the API server.
    async fn bind(mut self) -> Self {
        let Some(ref node) = self.chosen else {
            return self;
        };

        // make patch call to api server
        let patch = PodPatch {
            pod_field: PodField::NodeName,
            value: Value::String(node.clone()),
        };

        let client = Client::new();
        let base_url = self
            .state
            .api_server
            .as_deref()
            .unwrap_or("http://localhost:7620");
        let url = format!("{}/pods/{}", base_url, self.pod.metadata.name);

        match client.patch(&url).json(&patch).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    pod=%self.pod.metadata.name,
                    %node,
                    "Scheduled"
                );
                self.accepted = true;
            }
            Ok(resp) => {
                tracing::error!(
                    status = %resp.status(),
                    "Failed to patch pod: non-success response"
                );
            }
            Err(err) => {
                tracing::error!("Failed to patch pod: {}", err);
            }
        }
        self
    }
}
