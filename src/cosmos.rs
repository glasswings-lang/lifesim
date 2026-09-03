//! The first act: from the Planck instant to the first collapsing gas cloud.
//!
//! Each universe draws its own constants. That matters, and it is the part most
//! simulators skip: a cosmos with too much dark energy tears itself smooth
//! before anything can clump, and a cosmos with too many baryons per photon
//! burns most of its hydrogen into helium in the first twenty minutes and never
//! gets long-lived stars. Most of the possible universes are boring. Ours had
//! to be drawn from the small set that is not, and so does every run of this.
//!
//! The expansion history is a real numerical integration of the Friedmann
//! equation. The nucleosynthesis yields are fits to full reaction-network
//! results. The structure formation is Press-Schechter: linear growth of
//! density peaks until they cross the spherical-collapse threshold. All of it
//! is approximate. None of it is decoration.

use crate::rng::Rng;
use crate::units::*;
use crate::narrate::{Scribe, Detail};

pub struct Cosmos {
    #[allow(dead_code)]
    pub seed: u64,
    // drawn parameters
    pub h0: f64,          // km/s/Mpc
    pub omega_m: f64,     // matter density
    pub omega_l: f64,     // dark energy density
    pub omega_r: f64,     // radiation density
    pub eta10: f64,       // baryons per 10^10 photons
    pub sigma8: f64,      // clumpiness of the initial field
    pub omega_b: f64,     // baryon density

    // derived
    pub y_helium: f64,    // primordial helium mass fraction
    pub x_hydrogen: f64,
    pub d_over_h: f64,    // deuterium abundance
    pub li7: f64,
    pub z_eq: f64,        // matter-radiation equality
    pub t_rec_yr: f64,    // recombination
    pub z_first: f64,     // first collapsing halos
    pub t_first_yr: f64,
    pub age_now_yr: f64,
    pub viable: bool,
    pub epitaph: String,
}

impl Cosmos {
    /// Hubble parameter at scale factor a, in inverse seconds.
    pub fn hubble(&self, a: f64) -> f64 {
        let ok = 1.0 - self.omega_m - self.omega_l - self.omega_r;
        let e2 = self.omega_r / a.powi(4)
            + self.omega_m / a.powi(3)
            + ok / (a * a)
            + self.omega_l;
        if e2 <= 0.0 { return 0.0; }
        h0_si(self.h0) * e2.sqrt()
    }

    /// Cosmic time at scale factor a, in years. Integrated, not fitted.
    pub fn age_at(&self, a: f64) -> f64 {
        let n = 3000;
        let lo = (1e-12f64).ln();
        let hi = a.max(1e-12).ln();
        let dl = (hi - lo) / n as f64;
        let mut t = 0.0;
        for i in 0..n {
            // dt = da / (a H) = d(ln a) / H
            let la = lo + (i as f64 + 0.5) * dl;
            let h = self.hubble(la.exp());
            if h <= 0.0 { return f64::INFINITY; }
            t += dl / h;
        }
        t / YEAR
    }

    /// Linear growth factor, normalised so D(a=1) = 1. This is what decides
    /// whether a density ripple ever becomes a galaxy.
    pub fn growth_raw(&self, x: f64) -> f64 {
        let n = 600;
        let lo = (1e-8f64).ln();
        let hi = x.max(1e-8).ln();
        let dl = (hi - lo) / n as f64;
        let mut s = 0.0;
        for i in 0..n {
            let la = lo + (i as f64 + 0.5) * dl;
            let aa = la.exp();
            let h = self.hubble(aa) / h0_si(self.h0);
            if h <= 0.0 { return 0.0; }
            // integrand da / (a H)^3, written in ln a
            s += dl / (aa * aa * h * h * h);
        }
        s * self.hubble(x) / h0_si(self.h0)
    }

    pub fn growth(&self, a: f64) -> f64 {
        let d0 = self.growth_raw(1.0);
        if d0 <= 0.0 { return 0.0; }
        self.growth_raw(a) / d0
    }

    /// RMS density fluctuation on the mass scale M (solar masses).
    /// A power-law fit to the cold dark matter transfer function: crude at the
    /// extremes, honest in the middle.
    pub fn sigma_m(&self, m_solar: f64) -> f64 {
        let m8 = 6.0e14 * (self.omega_m / 0.315);
        self.sigma8 * (m_solar / m8).powf(-0.122)
    }

    /// Redshift at which a nu-sigma peak of mass M collapses, if it ever does.
    pub fn collapse_z(&self, m_solar: f64, nu: f64) -> Option<f64> {
        const DELTA_C: f64 = 1.686;
        let need = DELTA_C / (nu * self.sigma_m(m_solar));
        if need > 1.3 { return None; }
        let mut lo = 1e-4f64;
        let mut hi = 1.0f64;
        for _ in 0..50 {
            let mid = (lo * hi).sqrt();
            if self.growth(mid) < need { lo = mid; } else { hi = mid; }
        }
        let a = (lo * hi).sqrt();
        Some(1.0 / a - 1.0)
    }
}

/// Draw a universe. Most of the variation is modest. Occasionally it is not.
pub fn birth(seed: u64, rng: &mut Rng) -> Cosmos {
    let h0 = rng.gauss(67.4, 4.0).max(20.0);
    let omega_m = if rng.chance(0.12) {
        rng.range(0.02, 0.95)
    } else {
        (0.315 * rng.lognormal(1.0, 0.10)).clamp(0.05, 0.9)
    };
    // Very nearly flat, as inflation insists, but not exactly.
    let curvature = rng.gauss(0.0, 0.004);
    let omega_r = 9.2e-5 * (67.4 / h0).powi(2);
    let omega_l = 1.0 - omega_m - omega_r - curvature;

    let eta10 = 6.14 * rng.lognormal(1.0, 0.13);
    let sigma8 = rng.gauss(0.811, 0.10).max(0.02);
    let omega_b = 0.0493 * (eta10 / 6.14) * (67.4 / h0).powi(2);

    // Nucleosynthesis. Helium rises slowly with baryon density; deuterium falls
    // steeply, because a denser universe burns it away.
    let y_helium = (0.2470 + 0.0100 * (eta10 / 6.14).ln()).clamp(0.05, 0.85);
    let d_over_h = 2.53e-5 * (6.14 / eta10).powf(1.6);
    let li7 = 5.0e-10 * (eta10 / 6.14).powf(2.1);
    let x_hydrogen = 1.0 - y_helium;

    let mut c = Cosmos {
        seed, h0, omega_m, omega_l, omega_r, eta10, sigma8, omega_b,
        y_helium, x_hydrogen, d_over_h, li7,
        z_eq: 0.0, t_rec_yr: 0.0, z_first: 0.0, t_first_yr: 0.0,
        age_now_yr: 0.0, viable: true, epitaph: String::new(),
    };

    c.z_eq = c.omega_m / c.omega_r - 1.0;
    // Recombination depends only weakly on the parameters: the temperature at
    // which hydrogen holds onto its electron is set by atomic physics.
    let z_rec = 1089.0 * (c.omega_b / 0.0493).powf(0.03);
    c.t_rec_yr = c.age_at(1.0 / (1.0 + z_rec));
    c.age_now_yr = c.age_at(1.0);

    // Can anything ever collapse? Ask for a 3-sigma peak on the smallest scale
    // that can cool: about a million solar masses of gas and dark matter.
    match c.collapse_z(1.0e6, 3.0) {
        Some(z) => {
            c.z_first = z;
            c.t_first_yr = c.age_at(1.0 / (1.0 + z));
        }
        None => {
            c.viable = false;
            c.epitaph = "Nothing ever collapsed. The density ripples were too \
                faint, or the expansion pulled them flat before gravity could \
                answer. This universe stayed a thinning fog, forever.".into();
        }
    }

    if c.omega_l < -0.02 && c.omega_m > 1.02 {
        let t_crunch = c.age_now_yr;
        if !t_crunch.is_finite() || t_crunch < 3e9 {
            c.viable = false;
            c.epitaph = "The expansion stalled and reversed. Everything fell \
                back together while the first stars were still forming, and \
                the sky closed like a hand.".into();
        }
    }
    if c.y_helium > 0.60 {
        c.viable = false;
        c.epitaph = format!(
            "Nucleosynthesis ran away. {} of the ordinary matter fused into \
             helium in the first twenty minutes, leaving too little hydrogen \
             for stars that burn slowly. Everything here lived fast and briefly.",
            pct(c.y_helium));
    }
    c
}

/// Narrate the first act, from before there was time to the first cold cloud.
pub fn tell_first_act(c: &Cosmos, s: &mut Scribe) {
    s.chapter("I. The First Three Minutes, and the Long Dark After");

    let opening = s.phrase(
        "There is no before. Time starts here, at the smallest interval that \
         means anything, and everything that will ever exist is inside a region \
         smaller than a proton and hotter than any number that will later have \
         a use.",
        "The simulation begins at the Planck time. All matter and energy \
         occupies a region far smaller than an atomic nucleus.");
    s.beat(0.0, &opening);

    s.aside(Detail::Deep,
        "At this instant the four forces are one thing. The temperature is \
         around 1.4e32 kelvin. Nothing in the later universe will ever be this \
         hot again, including the centre of every star that ever forms.");

    let t_planck_yr = 5.39e-44 / YEAR;
    let inflation = s.phrase(
        "Space stretches. Not the things in it, but the distance between them, \
         doubling and doubling again, faster than light is allowed to cross the \
         gap, because the rule about light does not apply to the stage itself. \
         In the time it takes for nothing at all to happen, the universe grows \
         by a factor larger than the number of atoms it will ever contain.",
        "Inflation. The scale factor increases by roughly 10^26 in about 1e-32 \
         seconds, flattening the geometry and stretching quantum fluctuations \
         to cosmic scale.");
    s.beat(t_planck_yr * 1e8, &inflation);
    s.say("Inflation ends. Whatever drove it decays into a bath of particles, \
           and the universe refills with heat at the cost of its own strangeness. \
           But it leaves fingerprints. The quantum jitter that existed when the \
           universe was small has been blown up to the size of the sky. Every \
           galaxy that will ever form is already sketched here, as a whisper of \
           density one part in a hundred thousand above the average.");
    s.fact(Detail::Normal, "depth of that whisper",
        &format!("{:.3} (ours measures 0.811)", c.sigma8));

    let quarks = s.phrase(
        "Quarks lose their freedom. Below two trillion kelvin they can no longer \
         fly apart, and the plasma condenses into protons and neutrons the way \
         steam condenses into rain. Matter and antimatter annihilate almost \
         perfectly. Almost. For every billion pairs that cancel, one particle of \
         matter is left standing, for reasons nobody has ever fully explained. \
         Everything that is ever built gets built out of that remainder.",
        "Quark-hadron transition. Protons and neutrons form. Matter-antimatter \
         annihilation leaves a residual matter excess of about one part per billion.");
    s.beat(1e-6 / YEAR, &quarks);

    let nuc = s.phrase("Nucleosynthesis.", "Big Bang nucleosynthesis begins.");
    s.beat(180.0 / YEAR, &format!(
        "{} Deuterium finally survives long enough to be built on, and over about \
         twenty minutes the universe fuses roughly {} of its ordinary matter into \
         helium. Then it expands past the density where fusion can continue and \
         stops, permanently, with the periodic table three entries long.",
        nuc, pct(c.y_helium)));
    s.fact(Detail::Normal, "hydrogen left over", &pct(c.x_hydrogen));
    s.fact(Detail::Normal, "deuterium per hydrogen", &format!("{:.2e}", c.d_over_h));
    s.fact(Detail::Deep, "lithium-7 per hydrogen", &format!("{:.2e}", c.li7));
    s.fact(Detail::Deep, "baryons per 10 billion photons", &format!("{:.2}", c.eta10));
    if c.y_helium > 0.35 {
        s.say("That is a great deal of helium. Stars here are born already \
               half-finished, and they will run hot and die young.");
    } else if c.y_helium < 0.18 {
        s.say("That is unusually little helium. This universe stays hydrogen \
               almost all the way down, and its stars will be patient ones.");
    }

    let z_eq = c.z_eq;
    s.beat(c.age_at(1.0 / (1.0 + z_eq)), &format!(
        "For fifty thousand years radiation has outweighed matter, and its \
         pressure has smoothed flat every clump that tried to form. Now the \
         balance tips. Matter wins, and for the first time gravity is allowed to \
         keep what it gathers. Redshift {:.0}.", z_eq));

    let rec = s.phrase(
        "The universe cools through three thousand kelvin, and electrons, for \
         the first time, are moving slowly enough to be caught. Every nucleus \
         takes one. Within a few tens of thousands of years the fog of loose \
         charge becomes clear neutral gas, and light, which has spent its entire \
         existence being knocked sideways after a few steps, suddenly finds the \
         road open in every direction at once. That light is still travelling. \
         It is the oldest thing anyone will ever see.",
        "Recombination. Electrons bind to nuclei, the universe becomes \
         transparent, and the cosmic microwave background is released.");
    s.beat(c.t_rec_yr, &rec);
    s.fact(Detail::Normal, "age at recombination", &years(c.t_rec_yr));
    s.fact(Detail::Deep, "expansion rate today", &format!("{:.1} km/s/Mpc", c.h0));

    if !c.viable {
        s.chapter("An Ending, Early");
        let ep = c.epitaph.clone();
        s.say(&ep);
        return;
    }

    let dark = s.phrase(
        "And then, nothing to see. The light of recombination reddens out of the \
         visible and the universe goes dark: no stars, no galaxies, only cold \
         hydrogen and helium drifting, and the slow, patient, invisible work of \
         gravity pulling on ripples too faint to notice. This goes on for a \
         hundred million years. It is the longest quiet the universe will ever have.",
        "The dark ages. No luminous sources exist. Density perturbations grow \
         under gravity.");
    s.beat(c.t_rec_yr * 1.4, &dark);

    let give = s.phrase("Something gives.", "First halo collapse.");
    s.beat(c.t_first_yr, &format!(
        "{} The first ripples finish their fall. At redshift {:.0}, clouds of \
         around a million solar masses reach the density where they can hold \
         themselves together, and molecular hydrogen, the only coolant this \
         chemistry-poor universe has, lets them shed heat and keep shrinking.",
        give, c.z_first));
    s.fact(Detail::Normal, "first structure at", &years(c.t_first_yr));
}
