/// Controls how aggressively the walker prunes paths.
/// Without limits, complex functions could generate millions of paths.
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Stop exploring a path after this many blocks. Default: 32.
    pub max_depth: usize,
    /// Stop unrolling a loop after this many iterations per path. Default: 3.
    pub max_loop_unroll: usize,
    /// Stop enumerating paths for a function after this many. Default: 10_000.
    pub max_paths: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_loop_unroll: 3,
            max_paths: 10_000,
        }
    }
}
