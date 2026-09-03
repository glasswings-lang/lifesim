//! Physical constants and human-readable formatting.
//!
//! Everything inside the simulation is SI unless a name says otherwise.
//! Everything the reader sees gets translated back into things a person can
//! hold: suns, earths, years, degrees.

#![allow(dead_code)]

// --- fundamental constants (CODATA, SI) ---
pub const G: f64 = 6.674_30e-11;          // gravitational constant, m^3 kg^-1 s^-2
pub const C: f64 = 2.997_924_58e8;        // speed of light, m/s
pub const H_PLANCK: f64 = 6.626_070_15e-34;
pub const K_B: f64 = 1.380_649e-23;       // Boltzmann, J/K
pub const SIGMA_SB: f64 = 5.670_374_419e-8; // Stefan-Boltzmann, W m^-2 K^-4
pub const M_PROTON: f64 = 1.672_621_924e-27;
pub const A_RAD: f64 = 7.565_733e-16;     // radiation constant, J m^-3 K^-4

// --- astronomical scales ---
pub const M_SUN: f64 = 1.988_41e30;       // kg
pub const R_SUN: f64 = 6.957e8;           // m
pub const L_SUN: f64 = 3.828e26;          // W
pub const T_SUN: f64 = 5772.0;            // K, effective temperature
pub const M_EARTH: f64 = 5.972_17e24;     // kg
pub const R_EARTH: f64 = 6.371e6;         // m
pub const M_JUP: f64 = 1.898_2e27;        // kg
pub const AU: f64 = 1.495_978_707e11;     // m
pub const PARSEC: f64 = 3.085_677_581e16; // m
pub const LY: f64 = 9.460_730_472e15;     // m
pub const YEAR: f64 = 3.155_693e7;        // s, Julian year
pub const MYR: f64 = YEAR * 1e6;
pub const GYR: f64 = YEAR * 1e9;

// --- cosmology (Planck 2018 baseline; individual universes vary around this) ---
pub const H0_KMS_MPC: f64 = 67.4;
pub const T_CMB_NOW: f64 = 2.7255;        // K

/// Hubble constant in inverse seconds.
pub fn h0_si(h0_kms_mpc: f64) -> f64 {
    h0_kms_mpc * 1000.0 / (1e6 * PARSEC)
}

// --- formatting ---

/// A number of years, told the way a person would say it.
pub fn years(y: f64) -> String {
    if y < 1e-9 {
        // Sub-nanosecond: we're in the first instants, so speak in seconds.
        return seconds(y * YEAR);
    }
    if y < 1.0 { return seconds(y * YEAR); }
    if y < 1e3 { return format!("{:.0} years", y); }
    if y < 1e6 { return format!("{:.0} thousand years", y / 1e3); }
    if y < 1e9 { return format!("{:.1} million years", y / 1e6); }
    format!("{:.2} billion years", y / 1e9)
}

/// Short form for timeline stamps, e.g. "13.80 Gyr".
pub fn stamp(y: f64) -> String {
    if y < 1.0 { return seconds(y * YEAR); }
    if y < 1e3 { return format!("{:.0} yr", y); }
    if y < 1e6 { return format!("{:.0} kyr", y / 1e3); }
    if y < 1e9 { return format!("{:.1} Myr", y / 1e6); }
    format!("{:.3} Gyr", y / 1e9)
}

pub fn seconds(s: f64) -> String {
    if s <= 0.0 { return "the first instant".into(); }
    if s < 1e-30 { return format!("{:.0e} seconds", s); }
    if s < 1e-6 { return format!("{:.0e} seconds", s); }
    if s < 1.0 { return format!("{:.3} seconds", s); }
    if s < 60.0 { return format!("{:.1} seconds", s); }
    if s < 3600.0 { return format!("{:.1} minutes", s / 60.0); }
    if s < 86400.0 { return format!("{:.1} hours", s / 3600.0); }
    format!("{:.1} days", s / 86400.0)
}

/// Temperature, in whatever unit makes it feel real at that scale.
pub fn temp(k: f64) -> String {
    if k > 1e9 { return format!("{:.1e} kelvin", k); }
    if k > 1e4 { return format!("{:.0} K", k); }
    if k > 400.0 { return format!("{:.0} K ({:.0} C)", k, k - 273.15); }
    format!("{:.1} C", k - 273.15)
}

pub fn mass_stellar(kg: f64) -> String {
    format!("{:.2} solar masses", kg / M_SUN)
}

pub fn mass_planet(kg: f64) -> String {
    let me = kg / M_EARTH;
    if me > 50.0 { format!("{:.2} Jupiters", kg / M_JUP) }
    else if me >= 0.1 { format!("{:.2} Earths", me) }
    else { format!("{:.3} Earths", me) }
}

pub fn distance(m: f64) -> String {
    if m > 0.5 * LY { return format!("{:.2} light years", m / LY); }
    if m > 0.01 * AU { return format!("{:.3} AU", m / AU); }
    format!("{:.0} thousand km", m / 1e6)
}

/// Round a fraction to a percentage a person can say out loud.
pub fn pct(x: f64) -> String {
    let p = (x * 100.0).max(0.0);
    if p < 0.005 { "essentially none".to_string() }
    else if p >= 10.0 { format!("{:.0}%", p) }
    else if p >= 1.0 { format!("{:.1}%", p) }
    else { format!("{:.2}%", p) }
}

/// Turn a small count into a word, because "3 planets" reads worse than "three".
pub fn count_word(n: usize) -> String {
    const W: [&str; 13] = ["no", "one", "two", "three", "four", "five", "six",
        "seven", "eight", "nine", "ten", "eleven", "twelve"];
    if n < W.len() { W[n].to_string() } else { n.to_string() }
}
