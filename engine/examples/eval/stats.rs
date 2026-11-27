//! Small statistics helpers: percentiles over seeds, and a seeded bootstrap for paired
//! comparisons. Values are `Option<f64>`. What `None` means depends on the metric
//! (`registry::Missing`): "did not happen before the run ended" (a ring never reached) is kept as
//! censored, not dropped; "nothing to measure on that seed" (a rate with no samples) is left out.
//! Summaries of metric ids go through `registry::summary`, which picks the reading.

/// Linear-interpolated percentile of sorted values (`p` in 0..=1).
pub fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let x = p * (sorted.len() - 1) as f64;
    let (lo, hi) = (x.floor() as usize, x.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (x - lo as f64)
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub n: usize,
    /// Seeds where the value exists (event happened).
    pub reached: usize,
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
}

/// Censored values count as "later than everything" (+inf), so a median over seeds where fewer
/// than half reached the event is +inf, read as "not reached".
pub fn summary(values: &[Option<f64>]) -> Summary {
    summary_as(values, false)
}

/// As `summary`, or, with `skip_missing`, over the seeds that have a value (`n` still counts all
/// seeds); with none, every percentile is +inf.
pub fn summary_as(values: &[Option<f64>], skip_missing: bool) -> Summary {
    let mut v: Vec<f64> = if skip_missing {
        values.iter().flatten().copied().collect()
    } else {
        values.iter().map(|x| x.unwrap_or(f64::INFINITY)).collect()
    };
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| {
        let x = pct(&v, p);
        if x.is_nan() {
            f64::INFINITY
        } else {
            x
        }
    };
    Summary {
        n: values.len(),
        reached: values.iter().filter(|x| x.is_some()).count(),
        p10: q(0.1),
        p50: q(0.5),
        p90: q(0.9),
    }
}

/// xorshift64*, so the bootstrap is reproducible.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as usize % n
    }
}

/// Median of paired differences and its 90% bootstrap interval (resampling seeds).
pub fn paired(diffs: &[f64]) -> (f64, f64, f64) {
    let mut d = diffs.to_vec();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = pct(&d, 0.5);
    let mut rng = Rng::new(d.len() as u64 + 7);
    let mut meds: Vec<f64> = (0..2000)
        .map(|_| {
            let mut s: Vec<f64> = (0..d.len()).map(|_| d[rng.below(d.len())]).collect();
            s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            pct(&s, 0.5)
        })
        .collect();
    meds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (med, pct(&meds, 0.05), pct(&meds, 0.95))
}
