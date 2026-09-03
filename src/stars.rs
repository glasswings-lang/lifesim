//! The second act: the first stars, the invention of every other element, and
//! the long chemical enrichment that has to happen before a rocky planet is
//! even a possibility.
//!
//! Nothing here is scripted. The initial mass function is sampled, the
//! mass-luminosity relation is the real broken power law, main-sequence
//! lifetimes fall out of mass over luminosity, and the elements accumulate in
//! the gas because specific stars died and gave them back. If this galaxy runs
//! out of gas early, it stays metal-poor, and its planets will be small and
//! iron-starved. That is a consequence, not a setting.

use crate::rng::Rng;
use crate::units::*;
use crate::narrate::{Scribe, Detail};

/// What is dissolved in the star-forming gas, in solar masses.
#[derive(Clone, Default)]
pub struct Abundances {
    pub h: f64,
    pub he: f64,
    pub c: f64,
    pub n: f64,
    pub o: f64,
    pub mg: f64,
    pub si: f64,
    pub s: f64,
    pub p: f64,
    pub fe: f64,
}

impl Abundances {
    pub fn metals(&self) -> f64 {
        self.c + self.n + self.o + self.mg + self.si + self.s + self.p + self.fe
    }
    pub fn total(&self) -> f64 { self.h + self.he + self.metals() }
    /// Metallicity: metals as a fraction of everything. The Sun is about 0.014.
    pub fn z(&self) -> f64 {
        let t = self.total();
        if t <= 0.0 { 0.0 } else { self.metals() / t }
    }
    /// [Fe/H], the astronomer's logarithmic iron scale. The Sun is 0.0.
    pub fn feh(&self) -> f64 {
        if self.fe <= 0.0 || self.h <= 0.0 { return -9.0; }
        ((self.fe / self.h) / (1.31e-3)).log10()
    }
    /// [O/Fe]: high when only massive stars have died, falling as white dwarfs
    /// start detonating. A real fossil record of a galaxy's pace.
    pub fn ofe(&self) -> f64 {
        if self.fe <= 0.0 || self.o <= 0.0 { return 0.0; }
        ((self.o / self.fe) / (4.4)).log10()
    }
    /// Rock-forming elements per hydrogen, relative to solar. This sets how
    /// much solid material a planet-forming disk gets.
    pub fn rock_ratio(&self) -> f64 {
        if self.h <= 0.0 { return 0.0; }
        let rock = self.mg + self.si + self.fe + self.o * 0.5;
        (rock / self.h) / 1.05e-2
    }
}

/// A single star, with the properties that follow from its mass and chemistry.
#[derive(Clone)]
pub struct Star {
    pub mass: f64,       // solar masses
    pub z: f64,          // metallicity
    pub lum: f64,        // solar luminosities, zero-age
    pub radius: f64,     // solar radii
    pub teff: f64,       // kelvin
    pub life_myr: f64,   // main-sequence lifetime
    #[allow(dead_code)]
    pub born_myr: f64,
    #[allow(dead_code)]
    pub abund: Abundances,
}

/// The Kroupa initial mass function: broken power law, sampled by rejection
/// against its own envelope. Small stars are overwhelmingly the common case,
/// which is why most of the sky is red dwarfs nobody can see.
pub fn sample_imf(rng: &mut Rng, m_min: f64, m_max: f64) -> f64 {
    loop {
        let m = rng.power_law(-1.3, m_min, m_max);
        let extra = if m < 0.5 { 1.0 } else { (m / 0.5).powf(-1.0) };
        if rng.unit() < extra { return m; }
    }
}

/// A top-heavy function for the first generation. With no metals to radiate
/// heat away, a collapsing cloud cannot fragment small, so the first stars were
/// monsters.
pub fn sample_imf_popiii(rng: &mut Rng) -> f64 {
    rng.power_law(-1.0, 10.0, 300.0)
}

/// Luminosity from mass: the observed broken power law for main-sequence stars.
pub fn luminosity(m: f64) -> f64 {
    if m < 0.43 { 0.23 * m.powf(2.3) }
    else if m < 2.0 { m.powi(4) }
    else if m < 55.0 { 1.4 * m.powf(3.5) }
    else { 32000.0 * m }
}

/// Radius from mass, roughly, for stars on the main sequence.
pub fn radius(m: f64) -> f64 {
    if m < 1.0 { m.powf(0.80) } else { m.powf(0.57) }
}

/// Effective temperature follows from luminosity and radius, by Stefan-Boltzmann.
pub fn teff(m: f64) -> f64 {
    let l = luminosity(m) * L_SUN;
    let r = radius(m) * R_SUN;
    (l / (4.0 * std::f64::consts::PI * r * r * SIGMA_SB)).powf(0.25)
}

/// Main-sequence lifetime: fuel over burn rate. The Sun gets ten billion years.
/// A star ten times heavier gets thirty million.
pub fn lifetime_myr(m: f64) -> f64 {
    10_000.0 * m / luminosity(m)
}

/// A one-word colour for a temperature, because "5772 kelvin" is not a picture.
pub fn colour(t: f64) -> &'static str {
    if t > 25000.0 { "a hard blue-white, painful to look at" }
    else if t > 10000.0 { "blue-white" }
    else if t > 7500.0 { "white" }
    else if t > 6000.0 { "yellow-white" }
    else if t > 5200.0 { "yellow" }
    else if t > 3700.0 { "orange" }
    else { "a deep, dim red" }
}

pub fn spectral_class(t: f64) -> &'static str {
    if t > 30000.0 { "O" } else if t > 10000.0 { "B" } else if t > 7500.0 { "A" }
    else if t > 6000.0 { "F" } else if t > 5200.0 { "G" } else if t > 3700.0 { "K" }
    else { "M" }
}

pub fn make_star(mass: f64, abund: &Abundances, born_myr: f64) -> Star {
    Star {
        mass,
        z: abund.z(),
        lum: luminosity(mass),
        radius: radius(mass),
        teff: teff(mass),
        life_myr: lifetime_myr(mass),
        born_myr,
        abund: abund.clone(),
    }
}

/// What a dying star gives back, in solar masses of each element.
/// Yields are order-of-magnitude fits to core-collapse nucleosynthesis models.
pub fn yields(m: f64, into: &mut Abundances) -> &'static str {
    if m < 0.9 {
        "" // still burning; nothing returned yet
    } else if m < 8.0 {
        // Low and intermediate mass: a planetary nebula, rich in carbon and
        // nitrogen dredged up from the shell, plus slow-neutron-capture metals.
        let ret = m * 0.45;
        into.h += ret * 0.68;
        into.he += ret * 0.30;
        into.c += ret * 0.010;
        into.n += ret * 0.004;
        into.o += ret * 0.004;
        "a planetary nebula"
    } else if m < 20.0 {
        // Core collapse. Oxygen and the alpha elements, and a neutron star.
        let ej = m - 1.4;
        into.h += ej * 0.45;
        into.he += ej * 0.32;
        into.o += ej * 0.09;
        into.c += ej * 0.015;
        into.n += ej * 0.003;
        into.mg += ej * 0.008;
        into.si += ej * 0.010;
        into.s += ej * 0.005;
        into.p += ej * 0.0002;
        into.fe += ej * 0.006;
        "a core-collapse supernova, leaving a neutron star"
    } else if m < 45.0 {
        let ej = m * 0.5;
        into.h += ej * 0.40;
        into.he += ej * 0.33;
        into.o += ej * 0.12;
        into.c += ej * 0.02;
        into.mg += ej * 0.010;
        into.si += ej * 0.014;
        into.s += ej * 0.007;
        into.p += ej * 0.0003;
        into.fe += ej * 0.004;
        "a supernova collapsing into a black hole"
    } else if m < 140.0 {
        "a direct collapse to a black hole, swallowing everything"
    } else {
        // Pair instability: the star is torn apart completely, leaving nothing
        // behind and enriching enormously.
        let ej = m;
        into.he += ej * 0.40;
        into.o += ej * 0.30;
        into.si += ej * 0.06;
        into.s += ej * 0.03;
        into.mg += ej * 0.03;
        into.fe += ej * 0.05;
        into.c += ej * 0.02;
        into.p += ej * 0.0005;
        "a pair-instability supernova that leaves nothing at all behind"
    }
}

/// A galaxy, followed as a single chemically-mixed reservoir. This is the
/// standard one-zone model: crude spatially, but it gets the *history* right,
/// and the history is what decides what planets can be made and when.
pub struct Galaxy {
    pub gas: f64,          // solar masses of gas available
    pub stars: f64,        // locked into stars
    pub abund: Abundances,
    pub age_myr: f64,
    pub ia_queue: Vec<(f64, f64)>, // (detonation time, mass of iron)
    pub sn_count: u64,
    pub popiii_done: bool,
    /// The average return of a single core-collapse supernova, integrated over
    /// the initial mass function once at the start. A real galaxy has millions
    /// of these per timestep; sampling each one individually would be honest
    /// and unusable, so we sample the distribution once and scale.
    pub mean_ccsn: Abundances,
}

impl Galaxy {
    pub fn new(gas: f64, y_he: f64) -> Self {
        let mut a = Abundances::default();
        a.h = gas * (1.0 - y_he);
        a.he = gas * y_he;
        Galaxy {
            gas, stars: 0.0, abund: a, age_myr: 0.0,
            ia_queue: Vec::new(), sn_count: 0, popiii_done: false,
            mean_ccsn: Abundances::default(),
        }
    }

    /// Integrate the yields over the mass function to get the average return
    /// per supernova. Done once; used forever after.
    pub fn calibrate(&mut self, rng: &mut Rng) {
        let n = 4000;
        let mut acc = Abundances::default();
        for _ in 0..n {
            let m = sample_imf(rng, 8.0, 100.0);
            yields(m, &mut acc);
        }
        let k = 1.0 / n as f64;
        acc.h *= k; acc.he *= k; acc.c *= k; acc.n *= k; acc.o *= k;
        acc.mg *= k; acc.si *= k; acc.s *= k; acc.p *= k; acc.fe *= k;
        self.mean_ccsn = acc;
    }

    /// Star formation rate, solar masses per year. Gas turns into stars on
    /// roughly a dynamical time, more efficiently when there is more of it.
    pub fn sfr(&self) -> f64 {
        if self.gas <= 0.0 { return 0.0; }
        0.05 * self.gas / 2.0e8
    }

    /// Advance the galaxy by dt (Myr), forming and killing stars.
    pub fn step(&mut self, dt: f64, rng: &mut Rng) {
        let formed = (self.sfr() * dt * 1e6).min(self.gas * 0.5);
        if formed <= 0.0 { return; }
        self.gas -= formed;
        self.stars += formed;
        self.age_myr += dt;

        // Massive stars die inside this step. Roughly one core-collapse
        // supernova per hundred solar masses of stars formed.
        let n_sn = formed / 100.0;
        if n_sn < 60.0 {
            // Few enough to matter individually, which is the case only in the
            // first generation, when the mass function is also different.
            let k = rng.poisson(n_sn);
            for _ in 0..k {
                let m = if self.abund.z() < 1e-6 && !self.popiii_done {
                    sample_imf_popiii(rng)
                } else {
                    sample_imf(rng, 8.0, 100.0)
                };
                let mut back = Abundances::default();
                yields(m, &mut back);
                self.add_gas(&back);
                self.sn_count += 1;
            }
        } else {
            let jitter = 1.0 + rng.gauss(0.0, 1.0) / n_sn.sqrt();
            self.add_scaled(&self.mean_ccsn.clone(), n_sn * jitter.max(0.0));
            self.sn_count += n_sn as u64;
        }
        if self.abund.z() > 1e-6 { self.popiii_done = true; }

        // Intermediate-mass stars return their envelopes on a longer delay:
        // planetary nebulae, rich in carbon and nitrogen.
        let ret = formed * 0.20;
        let mut env = Abundances::default();
        yields(3.0, &mut env);
        self.add_scaled(&env, ret / (3.0 * 0.45));

        // Type Ia supernovae: white dwarfs detonating long after their birth,
        // which is exactly why iron arrives late and oxygen arrives early.
        let n_ia = formed / 700.0;
        let batches = 8;
        for _ in 0..batches {
            let delay = 100.0 + rng.power_law(-1.0, 100.0, 8000.0);
            self.ia_queue.push((self.age_myr + delay, 0.7 * n_ia / batches as f64));
        }
        let now = self.age_myr;
        let mut due: Vec<f64> = Vec::new();
        self.ia_queue.retain(|&(t, fe)| {
            if t <= now { due.push(fe); false } else { true }
        });
        for fe in due {
            self.abund.fe += fe;
            self.abund.si += fe * 0.2;
            self.abund.s += fe * 0.1;
            self.gas += fe * 1.4;
            self.sn_count += (fe / 0.7) as u64;
        }
    }

    fn add_gas(&mut self, b: &Abundances) {
        self.abund.h += b.h; self.abund.he += b.he; self.abund.c += b.c;
        self.abund.n += b.n; self.abund.o += b.o; self.abund.mg += b.mg;
        self.abund.si += b.si; self.abund.s += b.s; self.abund.p += b.p;
        self.abund.fe += b.fe;
        self.gas += b.total();
    }

    fn add_scaled(&mut self, b: &Abundances, k: f64) {
        let mut c = b.clone();
        c.h *= k; c.he *= k; c.c *= k; c.n *= k; c.o *= k;
        c.mg *= k; c.si *= k; c.s *= k; c.p *= k; c.fe *= k;
        self.add_gas(&c);
    }
}

/// Run and narrate galactic history, and hand back the enrichment record so
/// that later acts know what the gas looked like at any moment.
pub fn tell_second_act(
    gal: &mut Galaxy, s: &mut Scribe, rng: &mut Rng, t_start_yr: f64,
) -> Vec<(f64, Abundances)> {
    s.chapter("II. First Light, and the Making of Everything Else");

    // The very first star: no metals, so no fragmentation, so enormous.
    let m3 = sample_imf_popiii(rng);
    let st3 = make_star(m3, &gal.abund, 0.0);
    let first = s.phrase(
        "The cloud falls in on itself and cannot stop. There is no carbon here, \
         no oxygen, no dust: nothing that can radiate away the heat of collapse \
         efficiently, so the gas stays warm, and warm gas will not break into \
         small pieces. It comes down in one enormous body.",
        "The first star forms from primordial gas. Without metal cooling the \
         cloud cannot fragment, producing a single very massive object.");
    s.beat(t_start_yr, &first);
    s.say(&format!(
        "It ignites. After a hundred million years of absolute darkness there is \
         a star: {} solar masses of it, burning {} times brighter than the Sun \
         will, at {}, {} — a colour no eye exists yet to be hurt by. The \
         ultraviolet pours out and strips electrons off the hydrogen for light \
         years in every direction. First light.",
        format!("{:.0}", m3), format!("{:.0}", st3.lum), temp(st3.teff),
        colour(st3.teff)));
    s.fact(Detail::Normal, "total lifetime before it dies",
        &format!("{:.1} million years of hydrogen burning", st3.life_myr));

    let mut fate_ab = Abundances::default();
    let fate = yields(m3, &mut fate_ab);
    s.beat(t_start_yr + st3.life_myr * 1e6, &format!(
        "It runs out of hydrogen in {} — less time than it took to form the \
         cloud it was born in — and ends as {}.",
        years(st3.life_myr * 1e6), fate));
    if m3 > 140.0 {
        s.say("Nothing survives it. The core got hot enough that photons began \
               turning into electron-positron pairs, the pressure holding the \
               star up went with them, and the whole thing collapsed and then \
               detonated in one stroke. Every atom it made is now moving \
               outward at ten thousand kilometres a second. This is the single \
               most generous thing that ever happens in a universe: a star that \
               keeps nothing.");
    } else {
        s.say("The debris expands into the dark, and for the first time there is \
               something in the universe heavier than lithium. Carbon. Oxygen. \
               Silicon. Iron. Every atom that will ever be in a rock, or in \
               water, or in anyone, is now loose in the gas, waiting.");
    }

    // Now the long grind of chemical evolution.
    let mut history: Vec<(f64, Abundances)> = Vec::new();
    let dt = 50.0;
    let mut announced_rock = false;
    let mut announced_solar = false;
    let mut t = 0.0f64;
    while t < 12_000.0 {
        gal.step(dt, rng);
        t += dt;
        if (t as i64) % 250 == 0 { history.push((t, gal.abund.clone())); }

        if !announced_rock && gal.abund.z() > 1.0e-4 {
            announced_rock = true;
            s.beat(t_start_yr + t * 1e6, &format!(
                "The gas crosses a threshold nobody set. There are now enough \
                 heavy atoms — about {} of a percent by mass — that dust grains \
                 can condense in cooling ejecta, and dust changes everything: it \
                 radiates heat away, so clouds can fragment, so stars can be \
                 born small, so stars can be born *slow*. And grains stick to \
                 grains. Rocks are possible now. [Fe/H] is {:.2}.",
                format!("{:.2}", gal.abund.z() * 100.0), gal.abund.feh()));
            s.say("Everything after this is downstream of that.");
        }
        if !announced_solar && gal.abund.z() > 0.013 {
            announced_solar = true;
            s.beat(t_start_yr + t * 1e6, &format!(
                "The gas reaches roughly the chemistry the Sun was born with. \
                 Iron has caught up with oxygen as the white dwarfs of earlier \
                 generations begin detonating: [O/Fe] has fallen to {:.2} from \
                 the alpha-rich values of the early galaxy. Phosphorus, which \
                 almost nothing makes and everything alive needs, is present at \
                 {:.1e} by mass.",
                gal.abund.ofe(), gal.abund.p / gal.abund.total().max(1e-30)));
        }
    }

    s.beat(t_start_yr + t * 1e6, &format!(
        "Twelve billion years of this. {} supernovae have gone off. {} of the \
         galaxy's original gas is locked into stars, {} still drifts free, and \
         what drifts is {} heavy elements by mass — {} times the enrichment the \
         Sun was born into.",
        format!("{:.2} billion", gal.sn_count as f64 / 1e9),
        pct(gal.stars / (gal.stars + gal.gas)),
        format!("{:.2e} solar masses", gal.gas),
        pct(gal.abund.z()),
        format!("{:.2}", gal.abund.z() / 0.014)));
    s.fact(Detail::Normal, "iron abundance [Fe/H]", &format!("{:+.2}", gal.abund.feh()));
    s.fact(Detail::Normal, "oxygen to iron [O/Fe]", &format!("{:+.2}", gal.abund.ofe()));
    s.fact(Detail::Deep, "rock-forming elements vs solar",
        &format!("{:.2}x", gal.abund.rock_ratio()));

    history
}
