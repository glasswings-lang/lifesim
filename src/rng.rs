//! Deterministic randomness.
//!
//! Every universe is a seed. The same seed must always grow the same cosmos,
//! on any machine, forever. So we carry our own generator rather than trusting
//! whatever the operating system feels like today.
//!
//! This is PCG-XSH-RR (O'Neill 2014): small, fast, and statistically sound.

#[derive(Clone)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut r = Rng { state: 0, inc: (seed << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed ^ 0x9E37_79B9_7F4A_7C15);
        r.next_u32();
        r
    }

    /// A child generator, so that (say) planet 3's biosphere can be rerolled
    /// without disturbing the star that lights it.
    pub fn fork(&mut self, tag: u64) -> Rng {
        Rng::new(self.next_u64() ^ tag.wrapping_mul(0xD1B5_4A32_D192_ED03))
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    pub fn next_u64(&mut self) -> u64 {
        ((self.next_u32() as u64) << 32) | self.next_u32() as u64
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.unit() * (hi - lo)
    }

    /// Uniform integer in [lo, hi).
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo { return lo; }
        lo + (self.next_u64() % ((hi - lo) as u64)) as i64
    }

    /// Standard normal, via Box-Muller.
    pub fn normal(&mut self) -> f64 {
        let u1 = self.unit().max(1e-300);
        let u2 = self.unit();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    pub fn gauss(&mut self, mean: f64, sd: f64) -> f64 {
        mean + sd * self.normal()
    }

    /// Log-normal: the natural shape for masses, sizes, and most things in
    /// astronomy that can't be negative and span orders of magnitude.
    pub fn lognormal(&mut self, median: f64, sigma_dex: f64) -> f64 {
        median * 10f64.powf(self.normal() * sigma_dex)
    }

    /// A power law p(x) ~ x^alpha over [lo, hi]. The workhorse for stellar
    /// masses, impactor sizes, and anything else built by fragmentation.
    pub fn power_law(&mut self, alpha: f64, lo: f64, hi: f64) -> f64 {
        let u = self.unit();
        if (alpha + 1.0).abs() < 1e-9 {
            lo * (hi / lo).powf(u)
        } else {
            let a = alpha + 1.0;
            (lo.powf(a) + u * (hi.powf(a) - lo.powf(a))).powf(1.0 / a)
        }
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Poisson draw, for counting rare events in a time step.
    pub fn poisson(&mut self, lambda: f64) -> u32 {
        if lambda <= 0.0 { return 0; }
        if lambda > 30.0 {
            return self.gauss(lambda, lambda.sqrt()).max(0.0).round() as u32;
        }
        let l = (-lambda).exp();
        let mut k = 0u32;
        let mut p = 1.0;
        loop {
            p *= self.unit();
            if p <= l { return k; }
            k += 1;
            if k > 500 { return k; }
        }
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
}
