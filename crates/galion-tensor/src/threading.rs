// Galion Voice — Perpetual Open MIT License
//! Thread-pool hook. Scalar path is always available; rayon path behind feature `parallel`.

/// Execution mode selected per call. `Auto` = parallel when the feature is on and work is large.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Exec {
    /// Single-threaded scalar reference path (ground truth for tests).
    Scalar,
    /// Rayon path if compiled in, else falls back to scalar.
    Parallel,
    #[default]
    Auto,
}

/// Minimum work units (rows*cols) before `Auto` goes parallel.
pub const PAR_THRESHOLD: usize = 16 * 1024;

impl Exec {
    pub fn use_parallel(self, work: usize) -> bool {
        cfg!(feature = "parallel")
            && match self {
                Exec::Scalar => false,
                Exec::Parallel => true,
                Exec::Auto => work >= PAR_THRESHOLD,
            }
    }
}

/// Configure the global pool once (e.g. VPS with 2 vCPU => `init_thread_pool(2)`).
/// Returns Ok(false) if the pool was already initialized or the feature is off.
pub fn init_thread_pool(num_threads: usize) -> Result<bool, String> {
    #[cfg(feature = "parallel")]
    {
        match rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .thread_name(|i| format!("galion-tensor-{i}"))
            .build_global()
        {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = num_threads;
        Ok(false)
    }
}

pub fn num_threads() -> usize {
    #[cfg(feature = "parallel")]
    { rayon::current_num_threads() }
    #[cfg(not(feature = "parallel"))]
    { 1 }
}

/// Apply `f(row_index, row_slice)` over `out` split into rows of `row_len`.
pub fn for_each_row(out: &mut [f32], row_len: usize, exec: Exec, f: impl Fn(usize, &mut [f32]) + Sync + Send) {
    if row_len == 0 { return; }
    #[cfg(feature = "parallel")]
    if exec.use_parallel(out.len()) {
        use rayon::prelude::*;
        out.par_chunks_mut(row_len).enumerate().for_each(|(i, r)| f(i, r));
        return;
    }
    let _ = exec;
    out.chunks_mut(row_len).enumerate().for_each(|(i, r)| f(i, r));
}
