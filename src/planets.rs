//! The third act: a disk of gas and dust around a young star, and what
//! gravity does to it over ten million years.
//!
//! The chain here is causal all the way down, and it is worth saying out loud
//! because it is the thing that makes this a simulation rather than a story
//! generator:
//!
//!   how much iron the galaxy has made
//!     -> how much solid dust is in the disk
//!       -> how big planetary embryos can grow before they run out of feeding
//!          zone (the isolation mass)
//!         -> whether any of them reach the ten Earth masses needed to start
//!            pulling in gas before the gas is gone
//!           -> whether there are giant planets
//!             -> whether the inner system survives, and whether anything
//!                delivers water to it
//!
//! Break any link and everything downstream changes. A metal-poor star gets
//! small dry rocks. A metal-rich one often gets a giant that migrates inward
//! and destroys everything in its path. The narrow middle is where worlds live.

use crate::rng::Rng;
use crate::units::*;
use crate::stars::Star;
use crate::narrate::{Scribe, Detail};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Rocky,
    Ocean,       // rock core under a deep water or ice mantle
    Ice,         // a cold ice-rock body
    IceGiant,
    GasGiant,
}

#[derive(Clone)]
pub struct Planet {
    pub a: f64,          // semi-major axis, AU
    pub mass: f64,       // Earth masses
    pub radius: f64,     // Earth radii
    pub ecc: f64,
    pub kind: Kind,
    pub ice_frac: f64,   // volatile fraction by mass
    pub iron_frac: f64,
    pub water: f64,      // oceans, in Earth-ocean units
    pub moons: u32,
    pub big_moon: bool,
    pub obliquity: f64,  // degrees
    pub day_hours: f64,
    pub tidally_locked: bool,
    pub tectonics: bool,
    pub magnetic_field: bool,
    pub t_eq: f64,       // equilibrium temperature, K
    pub t_surf: f64,     // with greenhouse, K
    pub pressure: f64,   // bar
    pub name: String,
}

impl Planet {
    pub fn period_days(&self, m_star: f64) -> f64 {
        365.25 * (self.a.powi(3) / m_star).sqrt()
    }
    pub fn gravity(&self) -> f64 {
        self.mass / (self.radius * self.radius)
    }
    pub fn escape_kms(&self) -> f64 {
        11.186 * (self.mass / self.radius).sqrt()
    }
}

/// Mass-radius relations, fitted to the observed exoplanet population.
fn radius_from(mass: f64, kind: Kind, ice: f64) -> f64 {
    match kind {
        Kind::Rocky => mass.powf(0.27),
        Kind::Ocean => mass.powf(0.27) * (1.0 + 0.9 * ice),
        Kind::Ice => mass.powf(0.27) * 1.4,
        Kind::IceGiant => 2.0 * mass.powf(0.22),
        Kind::GasGiant => {
            // Above roughly half a Jupiter, adding mass stops adding size:
            // electron degeneracy takes over and the planet just gets denser.
            let mj = mass / 317.8;
            11.2 * (1.0 / (1.0 + (mj / 0.5).powf(-1.4))).powf(0.12) * mj.powf(0.03).max(0.5)
        }
    }
}

/// Build a planetary system around a star. This runs the actual sequence:
/// dust settling, oligarchic growth to isolation mass, gas capture, migration,
/// then a chaotic giant-impact phase that merges what is left.
pub fn form_system(star: &Star, rock_ratio: f64, rng: &mut Rng) -> Vec<Planet> {
    // Disk mass scales with the star and varies a lot between stars.
    let disk_factor = rng.lognormal(1.4, 0.35);
    let t_disk_myr = rng.range(2.0, 10.0);   // how long the gas lasts

    // Where ice can condense. Inside this line water is vapour and the only
    // solids are rock and metal; outside it, solids roughly quadruple.
    let r_ice = 2.7 * star.lum.sqrt();
    let r_in = 0.02 * star.lum.powf(0.25).max(0.3);
    let r_out = 40.0 * star.mass.powf(0.5);

    // Solid surface density, in kg/m^2 at 1 AU, scaled by how much rock the
    // galaxy has managed to make by now.
    let sigma0 = 70.0 * disk_factor * rock_ratio.max(0.001);

    let sigma = |r: f64| -> f64 {
        let base = sigma0 * r.powf(-1.5);
        if r > r_ice { base * 4.2 } else { base }
    };

    // --- oligarchic growth ---
    // Lay down embryos, each spaced by ten mutual Hill radii, each growing to
    // the isolation mass of its own feeding zone.
    let mut embryos: Vec<Planet> = Vec::new();
    let mut r = r_in.max(0.03);
    let b = 10.0;
    let m_star_kg = star.mass * M_SUN;
    while r < r_out && embryos.len() < 40 {
        let a_m = r * AU;
        let s = sigma(r);
        // M_iso = (2 pi b a^2 Sigma)^(3/2) / (3 M*)^(1/2)
        let m_iso = (2.0 * std::f64::consts::PI * b * a_m * a_m * s).powf(1.5)
            / (3.0 * m_star_kg).sqrt();
        let m_e = m_iso / M_EARTH;
        if m_e > 1e-4 {
            let cold = r > r_ice;
            embryos.push(Planet {
                a: r,
                mass: m_e * rng.lognormal(1.0, 0.12),
                radius: 0.0,
                ecc: rng.range(0.0, 0.03),
                kind: if cold { Kind::Ice } else { Kind::Rocky },
                ice_frac: if cold { rng.range(0.3, 0.6) } else { 0.0 },
                iron_frac: (0.32 * rng.lognormal(1.0, 0.15)).clamp(0.05, 0.7),
                water: 0.0, moons: 0, big_moon: false,
                obliquity: 0.0, day_hours: 12.0, tidally_locked: false,
                tectonics: false, magnetic_field: false,
                t_eq: 0.0, t_surf: 0.0, pressure: 0.0,
                name: String::new(),
            });
        }
        // Step outward by the spacing of the zone we just consumed.
        let r_h = r * (m_iso / (3.0 * m_star_kg)).cbrt();
        r += (b * r_h).max(r * 0.18);
    }

    // --- gas capture ---
    // An embryo past about ten Earth masses, while gas remains, cannot stop:
    // the envelope it holds compresses under its own weight, which lets it hold
    // more, which compresses further. Runaway.
    for p in embryos.iter_mut() {
        if p.mass > 8.0 && p.a > r_ice * 0.8 {
            let reach = (t_disk_myr / 4.0) * rng.lognormal(1.0, 0.4);
            let gained = 40.0 * reach * (p.mass / 10.0).powf(0.5) * rng.lognormal(1.0, 0.5);
            p.mass += gained;
            p.kind = if p.mass > 60.0 { Kind::GasGiant } else { Kind::IceGiant };
            p.ice_frac = if p.kind == Kind::GasGiant { 0.02 } else { 0.25 };
        }
    }

    // --- migration ---
    // A giant carves a gap and then rides the gas inward. Sometimes it stops
    // at the disk's inner edge as a hot Jupiter, having eaten or ejected
    // everything it passed.
    let mut destroyed_inner = false;
    for i in 0..embryos.len() {
        if embryos[i].kind == Kind::GasGiant && rng.chance(0.30) {
            let target = (r_in * rng.range(1.5, 8.0)).max(0.02);
            if target < embryos[i].a * 0.5 {
                embryos[i].a = target;
                destroyed_inner = true;
            }
        } else if embryos[i].kind != Kind::GasGiant {
            // Small bodies drift too, more gently.
            embryos[i].a *= rng.lognormal(1.0, 0.05);
        }
    }
    if destroyed_inner {
        let cutoff = embryos.iter()
            .filter(|p| p.kind == Kind::GasGiant)
            .map(|p| p.a).fold(f64::INFINITY, f64::min);
        embryos.retain(|p| p.kind == Kind::GasGiant || p.a > cutoff * 1.6);
    }

    // --- the giant impact phase ---
    // Once the gas is gone there is nothing left to damp the orbits. Everything
    // that remains crosses everything else, and for a hundred million years the
    // inner system is a demolition derby. This is where Earth actually got made,
    // and where its Moon came from.
    embryos.sort_by(|x, y| x.a.partial_cmp(&y.a).unwrap());

    // Several rounds, because each merger widens the gaps and destabilises the
    // neighbours again. Earth took about a hundred million years of this.
    let mut impacts: Vec<u32> = vec![0; embryos.len()];
    for _round in 0..6 {
        let mut out: Vec<Planet> = Vec::new();
        let mut hits: Vec<u32> = Vec::new();
        let mut i = 0usize;
        while i < embryos.len() {
            let mut p = embryos[i].clone();
            let mut h = impacts[i];
            let mut j = i + 1;
            while j < embryos.len() {
                let q = &embryos[j];
                let sep = (q.a - p.a) / p.a;
                // Two bodies closer than about a third of their separation in
                // orbital radius cannot both survive; one eats the other.
                let crowded = sep < 0.33
                    && p.kind != Kind::GasGiant && q.kind != Kind::GasGiant
                    && rng.chance(0.7);
                if crowded {
                    let total = p.mass + q.mass;
                    p.a = (p.a * p.mass + q.a * q.mass) / total;
                    p.iron_frac = (p.iron_frac * p.mass + q.iron_frac * q.mass) / total;
                    p.ice_frac = (p.ice_frac * p.mass + q.ice_frac * q.mass) / total;
                    p.mass = total;
                    h += 1 + impacts[j];
                    j += 1;
                } else { break; }
            }
            out.push(p);
            hits.push(h);
            i = j.max(i + 1);
        }
        let stable = out.len() == embryos.len();
        embryos = out;
        impacts = hits;
        if stable { break; }
    }

    let mut settled: Vec<Planet> = Vec::new();
    for (i, mut p) in embryos.into_iter().enumerate() {
        let merged = impacts[i];
        // A late off-centre collision spins the survivor up, tips it over, and
        // can throw a disk of debris into orbit that becomes a large moon.
        if merged > 0 {
            p.obliquity = rng.gauss(0.0, 30.0).abs().min(179.0);
            p.day_hours = rng.range(5.0, 30.0);
            if rng.chance(0.28) {
                p.big_moon = true;
                p.moons = 1 + rng.int(0, 2) as u32;
            } else {
                p.moons = rng.int(0, 3) as u32;
            }
            p.ecc = rng.range(0.0, 0.12);
        } else {
            p.obliquity = rng.range(0.0, 25.0);
            p.day_hours = rng.range(8.0, 40.0);
            p.moons = if p.kind == Kind::GasGiant { rng.int(4, 80) as u32 }
                      else { rng.int(0, 2) as u32 };
            p.ecc = rng.range(0.0, 0.05);
        }
        settled.push(p);
    }

    // --- water delivery ---
    // Inner planets form dry: they condensed inside the ice line. Their water
    // arrives later, thrown inward from the cold outer system by the gravity of
    // whatever giants exist. No giants, or a giant in the wrong place, and the
    // inner planets stay deserts.
    let giants: Vec<f64> = settled.iter()
        .filter(|p| matches!(p.kind, Kind::GasGiant | Kind::IceGiant))
        .map(|p| p.a).collect();
    let scatter_strength = if giants.is_empty() { 0.10 }
        else {
            let best = giants.iter().cloned().fold(0.0f64, |acc, g| {
                let ratio = g / r_ice;
                acc.max((1.0 / (1.0 + (ratio - 1.4).abs())).min(1.0))
            });
            best * rng.range(0.4, 1.8)
        };
    for p in settled.iter_mut() {
        if p.a < r_ice && matches!(p.kind, Kind::Rocky) {
            p.water = (scatter_strength * rng.lognormal(1.5, 0.55)).max(0.0);
            if p.water > 40.0 { p.kind = Kind::Ocean; p.ice_frac = 0.15; }
        } else if p.a >= r_ice {
            p.water = 500.0 * p.mass;
        }
    }

    // --- finishing properties ---
    for (n, p) in settled.iter_mut().enumerate() {
        p.radius = radius_from(p.mass, p.kind, p.ice_frac);
        // Plate tectonics needs enough internal heat to keep the mantle moving
        // and a thin enough lid for it to break: too small and it freezes,
        // too large and the crust is too thick to subduct.
        p.tectonics = p.mass > 0.35 && p.mass < 6.0
            && matches!(p.kind, Kind::Rocky | Kind::Ocean)
            && rng.chance(0.7);
        // A magnetic field needs a liquid metal core and rotation to stir it.
        p.magnetic_field = p.iron_frac > 0.15 && p.mass > 0.25
            && p.day_hours < 200.0 && rng.chance(0.75);
        p.name = if n < 25 {
            ((b'b' + n as u8) as char).to_string()
        } else {
            format!("{}{}", (b'a' + (n / 25) as u8) as char,
                            (b'b' + (n % 25) as u8) as char)
        };
    }

    settled
}

/// Tidal locking: how long before a planet's rotation matches its orbit.
/// Close in around a small star this takes far less time than the star lives,
/// which is why the most common habitable-zone planets in the universe
/// probably have a permanent day side and a permanent night side.
pub fn lock_time_myr(p: &Planet, m_star: f64) -> f64 {
    let a = p.a;
    6.0 * a.powi(6) * (p.radius / p.mass) / (m_star * m_star) * 1e3
}

/// Equilibrium and surface temperature. The greenhouse term is where a planet
/// stops being a rock and starts being a climate.
pub fn set_climate(p: &mut Planet, star: &Star, star_age_myr: f64, rng: &mut Rng) {
    // Stars brighten as they age: the Sun is about 30% brighter now than when
    // Earth formed. Any world that stays habitable has to survive that.
    let frac = (star_age_myr / star.life_myr).clamp(0.0, 1.0);
    let lum = star.lum * (1.0 + 0.6 * frac);

    let albedo = match p.kind {
        Kind::GasGiant | Kind::IceGiant => 0.35,
        Kind::Ice => 0.6,
        _ => 0.3,
    };
    let flux = lum / (p.a * p.a);  // in Earth-at-1-AU units
    p.t_eq = 278.5 * (flux * (1.0 - albedo) / 0.7).powf(0.25);

    if matches!(p.kind, Kind::GasGiant | Kind::IceGiant) {
        p.t_surf = p.t_eq;
        p.pressure = 1e6;
        return;
    }

    // Atmosphere: outgassed by volcanism, lost to space by heating. Small
    // worlds cannot hold on; large ones hold far too much.
    let esc = p.escape_kms();
    let xuv = (1.0 / (1.0 + (star_age_myr / 300.0))).max(0.05);
    let retain = (1.0 / (1.0 + (12.0 / esc).powf(4.0) * xuv * 8.0)).clamp(0.0, 1.0);
    let outgas = p.mass.powf(0.8) * if p.tectonics { 1.4 } else { 0.5 };
    let mut pressure = outgas * retain * rng.lognormal(1.0, 0.4);

    // The carbonate-silicate thermostat. On a world with liquid water and
    // moving plates, rain pulls carbon dioxide out of the air and volcanoes put
    // it back, and the rate of the first depends on temperature. It is a
    // negative feedback with a several-hundred-thousand-year time constant, and
    // it is the single reason a planet can stay temperate while its star
    // steadily brightens.
    if p.tectonics && p.water > 0.05 {
        let target = 288.0;
        for _ in 0..60 {
            let g = greenhouse_k(pressure, p.water);
            let t = p.t_eq + g;
            let err = t - target;
            pressure *= (1.0 - 0.08 * err / 40.0).clamp(0.5, 1.6);
            pressure = pressure.clamp(1e-4, 400.0);
        }
    }
    p.pressure = pressure;
    p.t_surf = p.t_eq + greenhouse_k(pressure, p.water);

    // Runaway states. Both are one-way doors.
    if p.t_surf > 340.0 && p.water > 0.0 {
        // Water vapour is itself a greenhouse gas, so a hot ocean makes a
        // hotter sky, which evaporates more ocean. Venus.
        p.t_surf += 180.0 * (p.water.min(3.0));
        p.pressure += 60.0;
        p.water *= 0.02;
    }
    if p.t_surf < 250.0 && p.water > 0.05 {
        // Ice reflects sunlight, which cools the planet, which makes more ice.
        p.t_surf -= 25.0;
    }
}

/// Greenhouse warming in kelvin, from atmospheric pressure and available water.
/// A saturating log law: doubling a thin atmosphere matters enormously,
/// doubling a thick one much less.
fn greenhouse_k(pressure_bar: f64, water: f64) -> f64 {
    if pressure_bar <= 1e-5 { return 0.0; }
    let base = 33.0 * (1.0 + pressure_bar).ln() / (2.0f64).ln();
    let vapour = 12.0 * (1.0 + water.min(5.0)).ln();
    base + vapour
}

pub fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Rocky => "rocky",
        Kind::Ocean => "an ocean world",
        Kind::Ice => "an ice-rock world",
        Kind::IceGiant => "an ice giant",
        Kind::GasGiant => "a gas giant",
    }
}

/// Narrate a system into being.
pub fn tell_third_act(
    star: &Star, sys: &[Planet], s: &mut Scribe, star_name: &str,
) {
    s.chapter(&format!("III. {}", star_name));

    s.say(&format!(
        "A cloud a few light years across, cold enough that its hydrogen has \
         paired into molecules, gets nudged — by a passing spiral arm, by the \
         shockwave of a nearby supernova, by nothing much at all — and part of \
         it stops being able to hold itself up.",
    ));
    s.say(&format!(
        "It falls. Because it was turning, even slightly, it cannot fall \
         straight: angular momentum has to go somewhere, so the collapse \
         flattens into a disk with a swelling knot at the centre. The knot \
         reaches ten million kelvin and hydrogen begins fusing into helium. \
         That is a star, and this one has {} solar masses, shines at {} suns, \
         and burns at {}. It is {}. On sight, from far enough away to survive \
         it, it looks {}.",
        format!("{:.2}", star.mass), format!("{:.3}", star.lum),
        temp(star.teff), format!("a {}-type star", crate::stars::spectral_class(star.teff)),
        crate::stars::colour(star.teff)));
    s.fact(Detail::Normal, "expected lifetime",
        &format!("{} on the main sequence", years(star.life_myr * 1e6)));
    s.fact(Detail::Normal, "metallicity", &format!("{:.3} ({:.2}x solar)", star.z, star.z / 0.014));
    s.fact(Detail::Deep, "radius", &format!("{:.2} solar radii", star.radius));

    if sys.is_empty() {
        s.say("The disk disperses before anything can accumulate. There is not \
               enough solid material here to build with, and the star is left \
               alone with a thin ring of dust that blows away in its own light.");
        return;
    }

    s.say(&format!(
        "In the leftover disk, dust grains stick where they touch. Pebbles \
         become boulders become mountains become bodies with their own gravity, \
         and once a body has gravity it stops waiting for collisions to find it \
         and starts pulling them in. Ten million years later the gas is gone and \
         {} worlds remain.",
        count_word(sys.len())));

    for p in sys {
        let per = p.period_days(star.mass);
        let per_s = if per > 800.0 { format!("{:.1} years", per / 365.25) }
            else { format!("{:.0} days", per) };
        s.item(&format!(
            "Planet {} — {}, {}, {} across, orbiting at {:.3} AU with a year of {}.",
            p.name, kind_name(p.kind), mass_planet(p.mass * M_EARTH),
            format!("{:.2} Earth radii", p.radius), p.a, per_s));
        s.fact(Detail::Normal, "surface gravity", &format!("{:.2} g", p.gravity()));
        s.fact(Detail::Normal, "equilibrium temperature", &temp(p.t_eq));
        if !matches!(p.kind, Kind::GasGiant | Kind::IceGiant) {
            s.fact(Detail::Normal, "surface temperature", &temp(p.t_surf));
            s.fact(Detail::Normal, "atmosphere", &format!("{:.3} bar", p.pressure));
            s.fact(Detail::Normal, "water", &format!("{:.2} Earth oceans", p.water));
            s.fact(Detail::Deep, "iron fraction", &pct(p.iron_frac));
            s.fact(Detail::Deep, "plate tectonics", if p.tectonics { "yes" } else { "no" });
            s.fact(Detail::Deep, "magnetic field", if p.magnetic_field { "yes" } else { "no" });
            s.fact(Detail::Deep, "axial tilt", &format!("{:.0} degrees", p.obliquity));
            s.fact(Detail::Deep, "day length", &format!("{:.1} hours", p.day_hours));
        }
        if p.moons > 0 {
            s.fact(Detail::Normal, "moons", &format!("{}{}", p.moons,
                if p.big_moon { ", one of them large, torn out of the planet by a collision" } else { "" }));
        }
        if p.tidally_locked {
            s.say("  It is tidally locked. One hemisphere faces its star forever \
                   and the other has never seen it. Everything that happens here \
                   happens in the ring of permanent dawn between them.");
        }
    }
}
