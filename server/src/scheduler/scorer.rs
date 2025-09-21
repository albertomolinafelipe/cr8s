pub type Score = f64;

#[derive(Clone, Copy)]
pub enum Scorer {
    Basic,
    // Spread,
    // BinPack,
}

impl Scorer {
    /// Compute a score given pod_count, free_cpu, free_mem
    pub fn score(&self, pod_count: usize, free_cpu: u64, free_mem: u64) -> Score {
        match self {
            Scorer::Basic => {
                // normalize
                let cpu_score = free_cpu as f64 / 4000.0;
                let mem_score = free_mem as f64 / (8.0 * 1024.0 * 1024.0 * 1024.0);

                // pod_count dominates, CPU+mem break ties
                let frac = 0.5 * cpu_score + 0.5 * mem_score;
                -(pod_count as f64) + frac
            }
        }
    }
}
