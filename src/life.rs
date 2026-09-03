//! The fourth act: chemistry that copies itself, and four billion years of
//! consequences.
//!
//! There are no scripted outcomes here. Nothing in this file says "after two
//! billion years, invent multicellularity." What exists instead is:
//!
//!   * genomes made of genes, which mutate, duplicate, delete, get swapped
//!     between lineages, and occasionally get merged wholesale when one cell
//!     swallows another and does not digest it;
//!   * a map from genes to traits that is *gated* — a nervous system cannot be
//!     expressed without a body to put it in, a body cannot be expressed
//!     without the oxygen budget to run it, and oxygen does not exist until
//!     something starts making it;
//!   * a fitness function that is just energy income minus energy cost, matched
//!     against an environment;
//!   * an environment that life itself is continuously rewriting.
//!
//! Complexity shows up in these runs because it pays, in the conditions life
//! has already created for itself. When it does not pay, it does not show up,
//! and the world stays a mat of bacteria for eight billion years and then the
//! star kills it. Both outcomes happen. Neither is written down anywhere.

use crate::rng::Rng;
use crate::planets::Planet;
use crate::stars::Star;

// ---------------------------------------------------------------- channels --

pub const N_CH: usize = 16;
pub const CHEMO: usize = 0;   // eating chemical gradients: the oldest metabolism
pub const PHOTO: usize = 1;   // eating light
pub const O2TOL: usize = 2;   // surviving oxygen, which is corrosive
pub const AEROBIC: usize = 3; // using oxygen, which is enormously profitable
pub const HEAT: usize = 4;
pub const COLD: usize = 5;
pub const DRY: usize = 6;     // desiccation and ultraviolet resistance
pub const SIZE: usize = 7;
pub const MULTI: usize = 8;   // sticking together and specialising
pub const MOTIL: usize = 9;
pub const SENSE: usize = 10;
pub const NEURAL: usize = 11;
pub const SOCIAL: usize = 12;
pub const MANIP: usize = 13;  // limbs, grasping, making things
pub const FECUND: usize = 14; // many cheap offspring versus few expensive ones
pub const SYMB: usize = 15;   // tolerance for living inside or alongside others

/// Human names for the sixteen channels, in order. Kept as the readable index
/// of what the constants above mean.
#[allow(dead_code)]
pub const CH_NAMES: [&str; N_CH] = [
    "chemosynthesis", "phototrophy", "oxygen tolerance", "aerobic respiration",
    "heat tolerance", "cold tolerance", "drought and UV resistance", "body size",
    "multicellularity", "motility", "sensing", "nervous complexity",
    "sociality", "manipulation", "fecundity", "symbiosis",
];

// ------------------------------------------------------------------ genome --

#[derive(Clone)]
pub struct Gene {
    pub ch: u8,     // which trait channel it feeds
    pub val: f64,   // -1 to 1: what it pushes toward
    pub expr: f64,  // 0 to 2: how loudly
}

#[derive(Clone)]
pub struct Genome {
    pub genes: Vec<Gene>,
}

impl Genome {
    /// The first genome: short, sloppy, and only able to do one thing.
    pub fn seed(rng: &mut Rng) -> Genome {
        let mut genes = Vec::new();
        for _ in 0..6 {
            genes.push(Gene {
                ch: rng.int(0, N_CH as i64) as u8,
                val: rng.range(-0.4, 0.4),
                expr: rng.range(0.2, 0.8),
            });
        }
        // Whatever it is, it must at least be able to eat something.
        genes.push(Gene { ch: CHEMO as u8, val: rng.range(0.4, 0.9), expr: 1.0 });
        Genome { genes }
    }

    /// Raw trait values before any gating: a squashed weighted sum.
    ///
    /// The offset matters more than it looks. Without it, a genome carrying no
    /// genes at all for a capability would score one half - competent at
    /// everything by default, which would mean the first cell to exist was
    /// already halfway to photosynthesis. With it, a capability starts near
    /// zero and has to be *built*, by accumulating genes that push it, which is
    /// what makes the order of events in a run mean anything.
    pub fn raw(&self) -> [f64; N_CH] {
        const BIAS: f64 = 2.6;
        let mut acc = [0.0f64; N_CH];
        for g in &self.genes {
            acc[g.ch as usize] += g.val * g.expr;
        }
        let mut out = [0.0f64; N_CH];
        for i in 0..N_CH {
            out[i] = 1.0 / (1.0 + (BIAS - acc[i]).exp());
        }
        out
    }

    pub fn distance(&self, other: &Genome) -> f64 {
        let a = self.raw();
        let b = other.raw();
        let mut d = 0.0;
        for i in 0..N_CH { d += (a[i] - b[i]).powi(2); }
        d.sqrt() + ((self.genes.len() as f64 - other.genes.len() as f64).abs() * 0.02)
    }
}

/// Mutate a genome. Every operator here is one that real genomes actually use,
/// and the interesting one is duplication: a copied gene is free to drift,
/// because the original is still doing the old job. Almost every genuine
/// innovation in the history of life started as a redundant copy of something.
pub fn mutate(g: &Genome, rng: &mut Rng, rate: f64) -> Genome {
    let mut out = g.clone();

    for gene in out.genes.iter_mut() {
        if rng.chance(rate) { gene.val = (gene.val + rng.gauss(0.0, 0.22)).clamp(-1.5, 1.5); }
        if rng.chance(rate * 0.6) { gene.expr = (gene.expr + rng.gauss(0.0, 0.15)).clamp(0.0, 2.5); }
    }
    // Duplication, with a chance the copy wanders to a different job.
    if rng.chance(rate * 2.5) && out.genes.len() < 90 {
        let i = rng.int(0, out.genes.len() as i64) as usize;
        let mut copy = out.genes[i].clone();
        if rng.chance(0.35) { copy.ch = rng.int(0, N_CH as i64) as u8; }
        copy.val += rng.gauss(0.0, 0.15);
        out.genes.push(copy);
    }
    // Deletion. Carrying a gene costs something; unused ones get lost.
    if rng.chance(rate * 1.6) && out.genes.len() > 5 {
        let i = rng.int(0, out.genes.len() as i64) as usize;
        out.genes.remove(i);
    }
    // Whole-genome duplication: rare, and violent, and the origin of a
    // startling amount of complexity when it works.
    if rng.chance(rate * 0.02) && out.genes.len() < 45 {
        let copy: Vec<Gene> = out.genes.iter().map(|x| {
            let mut c = x.clone();
            c.expr *= 0.6;
            if rng.chance(0.25) { c.ch = rng.int(0, N_CH as i64) as u8; }
            c
        }).collect();
        out.genes.extend(copy);
    }
    out
}

// ------------------------------------------------------------------ traits --

/// Turn a genome into a body, in an environment.
///
/// This is where the gating lives, and the gating is the whole reason
/// complexity appears in a particular order without anyone ordering it.
/// A steep threshold. Below the midpoint this is nearly zero, above it nearly
/// one, and the region in between is narrow and unrewarding to sit in.
fn sharp(x: f64, mid: f64, k: f64) -> f64 {
    1.0 / (1.0 + (-k * (x - mid)).exp())
}

pub fn express(raw: &[f64; N_CH], o2: f64) -> [f64; N_CH] {
    let mut t = *raw;

    // Aerobic respiration is worthless without oxygen to respire.
    let o2avail = (o2 / 0.10).min(1.0);
    t[AEROBIC] *= o2avail;

    // A body larger than a few cells cannot be supplied by diffusion alone.
    // Multicellularity stays latent until there is an energy budget for it.
    let energy_budget = (t[AEROBIC] * 1.2).min(1.0);
    t[MULTI] *= (0.15 + 0.85 * energy_budget).min(1.0);

    // Being *slightly* multicellular is worth nothing. A clump of cells that
    // does not yet specialise is just a cell that is harder to feed, and this
    // is a fitness valley, not a slope: partway across it you are worse off
    // than you were on either side. Lineages do not climb into it on purpose.
    // They fall in, by drift or by a duplication that happens to land right,
    // and almost all of them fall back out. That is why it took three billion
    // years on Earth and why it takes a long time here too.
    let body = sharp(t[MULTI], 0.52, 13.0);

    // Size is bounded by the same budget, and by having a body plan at all.
    t[SIZE] *= 0.20 + 0.80 * body;

    // A nervous system is a body-wide signalling network, so it needs a body,
    // and a large one before it is worth much: there is nothing for a big brain
    // in a small animal to do. Neurons are also the most metabolically
    // expensive tissue there is, which is why almost nothing buys many.
    t[NEURAL] *= body * energy_budget * (0.15 + 0.85 * t[SIZE]);

    // Sensing scales with having something to move toward.
    t[SENSE] *= 0.3 + 0.7 * t[MOTIL];

    // Manipulation needs a body and control of it.
    t[MANIP] *= body * (0.2 + 0.8 * t[NEURAL]);

    // Social behaviour needs enough nervous system to recognise another of you.
    t[SOCIAL] *= t[NEURAL].powf(1.2);

    t
}

// ------------------------------------------------------------------ species --

#[derive(Clone)]
pub struct Species {
    pub id: u64,
    pub genome: Genome,
    /// What the genome says, after gating: the actual working body.
    pub tr: [f64; N_CH],
    /// What the genome says before gating: the machinery it is building and
    /// paying for, whether or not conditions let it be used.
    pub rw: [f64; N_CH],
    pub share: f64,       // fraction of the biosphere
    pub born: f64,        // Myr
    pub parent: u64,
    pub name: String,
    pub land: bool,
}

impl Species {
    pub fn describe(&self) -> String {
        let t = &self.tr;

        let size = if t[MULTI] < 0.25 {
            if t[SYMB] > 0.6 { "single-celled, and host to something smaller living inside it" }
            else { "single-celled" }
        } else if t[SIZE] < 0.30 { "barely more than a clump of cells" }
        else if t[SIZE] < 0.48 { "small, about a fingernail" }
        else if t[SIZE] < 0.66 { "hand-sized" }
        else if t[SIZE] < 0.82 { "large, the size of a dog" }
        else { "very large, and still getting larger" };

        let eats = if t[PHOTO] > 0.6 { "living on light" }
            else if t[PHOTO] > 0.35 { "part-way to living on light" }
            else if t[MOTIL] > 0.45 && t[SIZE] > 0.4 { "hunting" }
            else if t[CHEMO] > 0.55 { "living on the chemistry leaking out of the rock" }
            else if t[CHEMO] > 0.3 { "scraping a living off mineral gradients" }
            else { "eating whatever else is alive" };

        let breath = if t[AEROBIC] > 0.6 { "burning oxygen for sixteen times the energy" }
            else if t[AEROBIC] > 0.3 { "using oxygen, inefficiently" }
            else if t[O2TOL] > 0.6 { "merely surviving the oxygen" }
            else { "poisoned by oxygen, and hiding from it" };

        let clime = if t[HEAT] > 0.65 { ", in water hot enough to scald" }
            else if t[COLD] > 0.65 { ", in the cold" }
            else if t[DRY] > 0.6 { ", and able to dry out completely and come back" }
            else { "" };

        let mind = if t[NEURAL] > 0.72 {
            ", with a nervous system dense enough to hold a model of what is not              in front of it"
        } else if t[MANIP] > 0.5 { ", and it handles things" }
            else if t[NEURAL] > 0.45 { ", with a real nervous system" }
            else if t[NEURAL] > 0.2 { ", with the beginnings of nerves" }
            else { "" };

        let social = if t[SOCIAL] > 0.6 { ", living in groups that outlast their members" } else { "" };

        let move_ = if t[MOTIL] > 0.65 { "fast" }
            else if t[MOTIL] > 0.35 { "slow-moving" }
            else { "sessile" };

        let where_ = if self.land { "on land" } else { "in water" };

        format!("{}, {}, {}, {} {}{}{}{}",
            size, move_, eats, breath, where_, clime, mind, social)
    }

    pub fn is_producer(&self) -> bool { self.tr[PHOTO] > 0.5 || self.tr[CHEMO] > 0.5 }
    /// Something that makes a substantial part of its living by catching other
    /// organisms, rather than by sitting still and being fed by light or rock.
    pub fn is_predator(&self) -> bool {
        self.tr[MOTIL] > 0.45 && self.tr[SIZE] > 0.30 && self.tr[PHOTO] < 0.55
    }
}

/// Pronounceable, seeded, and consistent for a given lineage.
pub fn coin_name(rng: &mut Rng) -> String {
    const ON: [&str; 22] = ["th","k","s","m","r","v","l","n","d","p","br","kr","st","sh","tr","gl","z","f","h","y","w","ch"];
    const NU: [&str; 12] = ["a","e","i","o","u","ae","ei","ou","ia","au","yu","eo"];
    const CO: [&str; 14] = ["n","s","r","l","th","m","k","x","sh","ll","nn","rr","ph","st"];
    let syl = rng.int(2, 4);
    let mut s = String::new();
    for i in 0..syl {
        s.push_str(*rng.pick(&ON));
        s.push_str(*rng.pick(&NU));
        if i == syl - 1 && rng.chance(0.7) { s.push_str(*rng.pick(&CO)); }
    }
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => "Ai".into(),
    }
}

// -------------------------------------------------------------- environment --

pub struct Env {
    pub t_surf: f64,
    pub o2: f64,          // atmospheric fraction
    pub co2: f64,         // bar
    pub ch4: f64,         // bar, a strong greenhouse gas made by anaerobes
    pub light: f64,       // relative to Earth
    pub uv: f64,          // surface ultraviolet, 1.0 = lethal, falls with ozone
    pub vents: f64,       // chemical energy from the interior
    pub nutrients: f64,
    pub land_open: bool,
    pub land_frac: f64,
    #[allow(dead_code)]
    pub ocean: bool,
    /// How much of the biosphere currently makes its living by eating other
    /// living things. Once this is non-zero, being fast, large and alert stops
    /// being a luxury.
    pub pred_pressure: f64,
    /// How much edible, hard-to-flee biomass is currently lying around.
    pub prey_pool: f64,
}

/// The biosphere, and the planet it is slowly rebuilding.
/// A lineage that existed, whether or not it still does. Kept so that you can
/// ask what something came from and follow it back, which you cannot do if the
/// only record of a species is that it is currently alive.
#[derive(Clone)]
pub struct Archived {
    pub id: u64,
    pub parent: u64,
    pub name: String,
    pub born: f64,
    pub died: Option<f64>,
    pub desc: String,
    pub genes: usize,
    pub peak: f64,
}

pub struct Biosphere {
    pub species: Vec<Species>,
    /// Every lineage that ever appeared here, living or not.
    pub archive: Vec<Archived>,
    pub next_id: u64,
    pub env: Env,
    #[allow(dead_code)]
    pub total_biomass: f64,
    pub myr: f64,
    /// Dissolved iron in the oceans, and every other reduced thing oxygen will
    /// react with before it is allowed into the air. Oxygen production runs for
    /// something like a billion years while this drains, and during the whole
    /// of that time the atmosphere does not change at all. Then it runs out.
    pub rust: f64,
}

impl Biosphere {
    pub fn record(&mut self, sp: &Species) {
        self.archive.push(Archived {
            id: sp.id, parent: sp.parent, name: sp.name.clone(), born: sp.born,
            died: None, desc: sp.describe(), genes: sp.genome.genes.len(),
            peak: sp.share,
        });
    }
    pub fn find(&self, needle: &str) -> Option<&Archived> {
        let n = needle.to_lowercase();
        self.archive.iter().find(|a| a.name.to_lowercase() == n)
            .or_else(|| self.archive.iter().find(|a| a.name.to_lowercase().starts_with(&n)))
    }
    pub fn alive(&self, id: u64) -> Option<&Species> {
        self.species.iter().find(|s| s.id == id)
    }
}

// -------------------------------------------------------------- the reactor --

/// Can life start here at all? This is the honest part: nobody knows the odds,
/// so what this does is check that the *preconditions* are present and then
/// roll against a rate that is a guess. When a run reports no life, that is
/// usually the preconditions failing, and it will say which one.
pub fn abiogenesis_odds(p: &Planet, env: &Env) -> (f64, Vec<&'static str>) {
    let mut missing = Vec::new();
    let mut odds: f64 = 1.0;

    if p.water < 0.02 { missing.push("no liquid water"); odds = 0.0; }
    if env.t_surf < 250.0 { missing.push("too cold for liquid solvent"); odds *= 0.02; }
    if env.t_surf > 400.0 { missing.push("hot enough to break every long molecule"); odds = 0.0; }
    if !p.tectonics { missing.push("a dead interior, so no chemical gradients to eat"); odds *= 0.08; }
    if p.pressure < 0.02 { missing.push("almost no atmosphere"); odds *= 0.05; }
    if env.uv > 0.9 && p.pressure < 0.5 { missing.push("unshielded ultraviolet"); odds *= 0.3; }

    // Temperate, wet, geologically alive: the odds per million years are still
    // small, but over a billion years small adds up.
    (odds * 0.004, missing)
}

pub fn new_env(p: &Planet, star: &Star, age_myr: f64) -> Env {
    let frac = (age_myr / star.life_myr).clamp(0.0, 1.0);
    let lum = star.lum * (1.0 + 0.6 * frac);
    Env {
        t_surf: p.t_surf,
        o2: 1e-7,
        co2: (p.pressure * 0.9).min(20.0),
        ch4: 1e-6,
        light: (lum / (p.a * p.a)).min(4.0),
        uv: 0.95,
        vents: if p.tectonics { 1.0 } else { 0.15 },
        nutrients: (p.tectonics as i32 as f64) * 0.7 + 0.3,
        land_open: false,
        land_frac: if p.water > 3.0 { 0.05 } else { 0.3 },
        ocean: p.water > 0.02,
        pred_pressure: 0.0,
        prey_pool: 0.0,
    }
}

/// Fitness of one body in one environment, ignoring who else is there.
///
/// This is the part selection actually acts on within a population: can this
/// organism pay for itself. The community terms - competition, predation,
/// crowding - are applied separately, to whole lineages, in `step`.
pub fn solo_fitness(t: &[f64; N_CH], rw: &[f64; N_CH], genes: usize, land: bool, env: &Env, mean: &[f64; N_CH]) -> f64 {
    let light_here = if land { env.light } else { env.light * 0.55 };
    let photo = t[PHOTO] * light_here * env.nutrients;
    let chemo = t[CHEMO] * env.vents;

    // Hunting is the one income stream that a nervous system improves. An
    // animal that can predict where the food will be next does better than one
    // that can only see where it is now, and that is the entire reason brains
    // are ever worth their upkeep.
    //
    // Note that this is open to everything, not only to things that have
    // already given up photosynthesis. That matters: if eating other organisms
    // required abandoning light first, no lineage could ever cross to it,
    // because the crossing would begin with pure loss. Real cells cheat exactly
    // this way - a great many single-celled organisms photosynthesise *and*
    // engulf things, and the ones that got good at the second stopped bothering
    // with the first. The trade-off below is what makes that a slope rather
    // than a cliff: you cannot be excellent at both.
    let commitment = 1.0 - 0.75 * t[PHOTO];
    let hunt = t[MOTIL] * t[SENSE] * (0.25 + t[SIZE]) * (0.4 + 0.9 * t[NEURAL])
        * commitment * env.prey_pool * 2.4;

    let yield_mult = 1.0 + 15.0 * t[AEROBIC];
    // Working together, and using things that are not part of your body.
    let leverage = (1.0 + 0.45 * t[SOCIAL]) * (1.0 + 0.7 * t[MANIP] * t[NEURAL]);
    let gross = (photo + chemo + hunt) * yield_mult * leverage;

    let mut cost = 0.06
        + 0.20 * t[SIZE]
        + 1.5 * t[NEURAL] * t[NEURAL] * (0.4 + t[SIZE])
        + 0.14 * t[MOTIL]
        + 0.10 * t[MULTI]
        + 0.07 * t[SENSE]
        + 0.10 * t[MANIP];

    // Burning oxygen does not make an organism rich, it makes it *fast*. The
    // income goes up by a large factor and so does the bill: an animal that
    // respires aerobically has a basal metabolic rate an order of magnitude
    // above a fermenting cell of the same size, and it starves in days rather
    // than years. Without this the sixteen-fold energy yield would make every
    // expensive organ free, and every world would grow a brain.
    cost *= 1.0 + 6.0 * t[AEROBIC];

    // Every gene has to be copied every time the organism divides. Carrying
    // spare ones is a real expense, and lineages under pressure shed them:
    // this is why bacterial genomes are small and tidy rather than growing
    // forever.
    cost += 0.0016 * genes as f64;

    // And a lineage pays for the machinery it is carrying even when conditions
    // will not let it use it. Genes that do nothing get lost, which is why a
    // world cannot spend its anoxic aeon quietly accumulating the genome for an
    // animal and then deploy it the afternoon the oxygen arrives.
    cost += 0.16 * (rw[MULTI] + rw[NEURAL] + rw[MANIP] + rw[SIZE]);

    let t_opt = 273.0 + 5.0 + 70.0 * t[HEAT] - 25.0 * t[COLD];
    let width = 22.0 + 25.0 * (t[HEAT] + t[COLD]);
    let therm = (-((env.t_surf - t_opt).powi(2)) / (2.0 * width * width)).exp();
    let o2_harm = ((env.o2 - 0.02).max(0.0) * 9.0 * (1.0 - t[O2TOL])).min(0.95);
    let uv_harm = if land { (env.uv * (1.0 - t[DRY])).min(0.95) } else { 0.0 };
    let dry_harm = if land { (0.12 + 0.6 * (1.0 - t[DRY])).min(0.9) } else { 0.0 };

    // Not being eaten. Size, speed and knowing what is behind you all help,
    // and none of them were worth anything before something started hunting.
    let defence = (1.0 - 0.45 * t[MOTIL] - 0.30 * t[NEURAL] - 0.25 * t[SIZE]).max(0.05);
    let risk = env.pred_pressure * defence * 1.3;

    // Being different from everyone else is worth something, which is what
    // stops every lineage climbing the same peak and staying there. Measured
    // only on how and where a thing makes its living - deliberately not on
    // size or complexity, or novelty would be quietly subsidising brains.
    let mut sim = 0.0;
    for k in [PHOTO, CHEMO, HEAT, COLD, DRY] { sim += (t[k] - mean[k]).powi(2); }
    let distinct = 0.15 * (1.0 - (-sim * 5.0).exp());

    gross * therm * (1.0 - o2_harm) * (1.0 - uv_harm) * (1.0 - dry_harm)
        - cost - risk + distinct
}

/// One million years.
pub fn step(bio: &mut Biosphere, rng: &mut Rng, p: &Planet, star: &Star) {
    bio.myr += 1.0;
    let env_o2 = bio.env.o2;

    // --- express every genome in the current air ---
    for sp in bio.species.iter_mut() {
        sp.rw = sp.genome.raw();
        sp.tr = express(&sp.rw, env_o2);
    }

    // --- evolution within a lineage ---
    // A species does not have to split in order to change. Most of what happens
    // over four billion years is this: a population tries variants, keeps the
    // ones that pay for themselves, and drifts on the ones that do not matter.
    // Without this step a lineage would be frozen from the moment it appeared.
    let mut mean = [0.0f64; N_CH];
    for sp in bio.species.iter() {
        for k in 0..N_CH { mean[k] += sp.share * sp.tr[k]; }
    }
    for idx in 0..bio.species.len() {
        if !rng.chance(0.40) { continue; }
        let land = bio.species[idx].land;
        let cand = mutate(&bio.species[idx].genome, rng, 0.09);
        let craw = cand.raw();
        let ctr = express(&craw, env_o2);
        let f_new = solo_fitness(&ctr, &craw, cand.genes.len(), land, &bio.env, &mean);
        let f_old = solo_fitness(&bio.species[idx].tr, &bio.species[idx].rw,
                                 bio.species[idx].genome.genes.len(),
                                 land, &bio.env, &mean);
        // Selection, plus a little drift: not every fixation is an improvement,
        // and small populations fix bad variants all the time.
        if f_new > f_old || rng.chance(0.05) {
            bio.species[idx].genome = cand;
            bio.species[idx].tr = ctr;
            bio.species[idx].rw = craw;
        }
    }

    // --- fitness ---
    // Individual fitness, then the community terms on top: who eats you, and
    // who else is already doing your job.
    let n = bio.species.len();
    let mut fit = vec![0.0f64; n];
    let shares: Vec<f64> = bio.species.iter().map(|s| s.share).collect();

    // How much food there is to catch, which is not a free parameter. Every
    // calorie a consumer eats was fixed by something that ate light or rock
    // first, and each trophic step wastes most of what it takes. So the pool is
    // the actual primary production of this biosphere, divided among everything
    // currently trying to hunt in it. A world where everything turned predator
    // would have nothing to prey on, and this is the term that says so.
    let production: f64 = bio.species.iter()
        .map(|s| {
            let light = if s.land { bio.env.light } else { bio.env.light * 0.55 };
            s.share * (s.tr[PHOTO] * light * bio.env.nutrients
                       + s.tr[CHEMO] * bio.env.vents)
        })
        .sum();
    let hunting_effort: f64 = bio.species.iter()
        .map(|s| s.share * s.tr[MOTIL] * (0.25 + s.tr[SIZE]) * (1.0 - 0.75 * s.tr[PHOTO]))
        .sum();
    bio.env.prey_pool = production / (1.0 + 6.0 * hunting_effort);

    for i in 0..n {
        let s = &bio.species[i];
        let t = &s.tr;
        let mut net = solo_fitness(t, &s.rw, s.genome.genes.len(), s.land, &bio.env, &mean);

        // Crowding: lineages that make a living the same way get in each
        // other's way. This is what keeps a biosphere diverse instead of
        // collapsing to a single winner.
        // Note that j == i is included. A lineage that is everywhere is mostly
        // competing with itself, and that self-limitation is the whole reason a
        // biosphere holds many species instead of collapsing onto the single
        // best one. Leave it out and every run ends with one winner.
        let mut overlap = 0.0;
        for j in 0..n {
            let q = &bio.species[j];
            let mut d = 0.0;
            for k in [PHOTO, CHEMO, SIZE, MOTIL, AEROBIC] {
                d += (t[k] - q.tr[k]).powi(2);
            }
            overlap += (-d * 6.0).exp() * shares[j];
        }
        net -= overlap * 1.5;

        fit[i] = net;
    }

    // --- replicator dynamics: above-average does better, and that is all ---
    let mean: f64 = (0..n).map(|i| fit[i] * shares[i]).sum();
    for i in 0..n {
        let g = ((fit[i] - mean) * 0.12).clamp(-2.0, 2.0);
        bio.species[i].share *= g.exp();
    }
    let tot: f64 = bio.species.iter().map(|s| s.share).sum();
    if tot > 0.0 {
        for s in bio.species.iter_mut() { s.share /= tot; }
    }
    // Note who is gone, and how big anyone ever got, before dropping them.
    {
        let now = bio.myr;
        let living: Vec<(u64, f64, String, usize)> = bio.species.iter()
            .map(|s| (s.id, s.share, s.describe(), s.genome.genes.len())).collect();
        for a in bio.archive.iter_mut() {
            if let Some((_, share, desc, genes)) = living.iter().find(|(id, ..)| *id == a.id) {
                if *share > a.peak { a.peak = *share; a.desc = desc.clone(); a.genes = *genes; }
                a.died = None;
            } else if a.died.is_none() {
                a.died = Some(now);
            }
        }
    }
    bio.species.retain(|s| s.share > 8.0e-6 && s.share.is_finite());

    // --- the planet answers back ---
    update_planet(bio, p);

    // --- new lineages ---
    let cap = 260;
    let parents: Vec<usize> = (0..bio.species.len())
        .filter(|&i| bio.species[i].share > 0.004)
        .collect();
    for i in parents {
        let rate = 0.06 + 0.10 * bio.species[i].tr[FECUND];
        if !rng.chance(rate) { continue; }
        if bio.species.len() >= cap { break; }
        let mut child = bio.species[i].clone();
        child.genome = mutate(&child.genome, rng, 0.10);

        // Horizontal transfer: early life passes genes sideways constantly,
        // which is why the tree of life is a net down at the bottom.
        if rng.chance(0.16) && bio.species.len() > 2 {
            let d = rng.int(0, bio.species.len() as i64) as usize;
            if !bio.species[d].genome.genes.is_empty() {
                let gi = rng.int(0, bio.species[d].genome.genes.len() as i64) as usize;
                child.genome.genes.push(bio.species[d].genome.genes[gi].clone());
            }
        }

        // Endosymbiosis: one cell engulfs another and fails to digest it, and
        // the two become one thing with both toolkits. This happened at least
        // twice on Earth and is the single largest jump in the record.
        if rng.chance(0.0025) && bio.species[i].tr[SIZE] > 0.3 && bio.species[i].tr[SYMB] > 0.4 {
            let cands: Vec<usize> = (0..bio.species.len())
                .filter(|&j| j != i && bio.species[j].tr[SIZE] < bio.species[i].tr[SIZE] * 0.6)
                .collect();
            if !cands.is_empty() {
                let j = cands[(rng.next_u64() % cands.len() as u64) as usize];
                for g in bio.species[j].genome.genes.iter().take(14) {
                    let mut c = g.clone();
                    c.expr *= 0.8;
                    child.genome.genes.push(c);
                }
                child.name = format!("{}", coin_name(rng));
            }
        }

        // Colonising land, if the sky will let anything live there.
        if bio.env.land_open && !child.land && rng.chance(0.05) && child.tr[DRY] > 0.35 {
            child.land = true;
        }

        child.rw = child.genome.raw();
        child.tr = express(&child.rw, bio.env.o2);
        if bio.species[i].genome.distance(&child.genome) > 0.06 {
            child.id = bio.next_id;
            bio.next_id += 1;
            child.parent = bio.species[i].id;
            child.born = bio.myr;
            child.name = coin_name(rng);
            let take = bio.species[i].share * 0.22;
            bio.species[i].share -= take;
            child.share = take;
            bio.record(&child);
            bio.species.push(child);
        }
    }

    let _ = star;
}

/// Life edits the air, the air edits the climate, the climate edits life.
fn update_planet(bio: &mut Biosphere, p: &Planet) {
    let photo_total: f64 = bio.species.iter()
        .map(|s| s.share * s.tr[PHOTO]).sum();
    let aerobic_total: f64 = bio.species.iter()
        .map(|s| s.share * s.tr[AEROBIC]).sum();
    let anaerobe_total: f64 = bio.species.iter()
        .map(|s| s.share * s.tr[CHEMO] * (1.0 - s.tr[O2TOL])).sum();

    // Oxygen: made by photosynthesis, consumed by breathing and by rusting the
    // rocks. For a long time the rocks win, and the air stays anoxic while the
    // oceans quietly fill with rust. Only when the sinks are full does oxygen
    // start accumulating, and then it does so fast.
    let source = photo_total * 0.014;
    if bio.rust > 0.0 {
        // Everything produced goes straight into the sea and sinks as rust.
        bio.rust -= source;
        bio.env.o2 = (bio.env.o2 + source * 0.02).clamp(1e-9, 0.02);
    } else {
        let sink = aerobic_total * bio.env.o2 * 0.11 + 0.0004;
        bio.env.o2 = (bio.env.o2 + source - sink).clamp(1e-9, 0.45);
    }

    // Methane from anaerobes: a strong greenhouse gas that oxygen destroys.
    bio.env.ch4 = (anaerobe_total * 0.02 * (1.0 - (bio.env.o2 / 0.01).min(1.0))).max(1e-7);

    // The carbonate-silicate cycle, which is the planet's thermostat.
    // Volcanoes put carbon dioxide in at a roughly constant rate. Rain takes it
    // out, and rain only weathers rock when there is liquid water and warmth,
    // so the sink shuts off completely when the world freezes over. That is why
    // a snowball ends: the volcanoes keep going, nothing removes what they emit,
    // and after a few tens of millions of years the greenhouse breaks the ice.
    let volcanism = if p.tectonics { 0.0028 } else { 0.0004 };
    let warmth = ((bio.env.t_surf - 258.0) / 30.0).clamp(0.0, 3.0);
    let biotic = 1.0 + 1.6 * photo_total;   // roots and microbes attack rock faster
    let weathering = 0.035 * warmth * biotic;
    bio.env.co2 = (bio.env.co2 + volcanism - weathering * bio.env.co2).clamp(1e-5, 60.0);

    // Ozone. Needs oxygen, and once it exists the land stops being sterile.
    let ozone = (bio.env.o2 / 0.05).min(1.0);
    bio.env.uv = (0.95 * (1.0 - 0.93 * ozone)).max(0.02);
    bio.env.land_open = bio.env.uv < 0.25 && p.water > 0.05 && bio.env.land_frac > 0.02;

    // The greenhouse, recomputed from what is actually in the air now.
    let g = 33.0 * (1.0 + bio.env.co2).ln() / (2.0f64).ln()
        + 900.0 * bio.env.ch4
        + 12.0 * (1.0 + p.water.min(5.0)).ln();
    bio.env.t_surf = p.t_eq + g;

    bio.env.nutrients = (0.3 + 0.7 * (p.tectonics as i32 as f64)) * (1.0 + 0.4 * bio.env.o2);

    bio.env.pred_pressure = bio.species.iter()
        .filter(|x| x.is_predator())
        .map(|x| x.share * x.tr[MOTIL] * (0.4 + x.tr[SIZE]))
        .sum();
}

// --------------------------------------------------------------- milestones --

#[derive(Default)]
pub struct Reached {
    pub life: bool,
    pub photo: bool,
    pub oxygenic: bool,
    pub great_oxidation: bool,
    pub endosymbiosis: bool,
    pub multicell: bool,
    pub predation: bool,
    pub nerves: bool,
    pub land: bool,
    pub minds: bool,
    pub society: bool,
    pub tools: bool,
    pub civilisation: bool,
}

impl Species {
    /// What it is actually like: shape, surface, how it moves, what it would
    /// be to stand next to.
    ///
    /// The other description says what a creature *does* - hunts, breathes
    /// oxygen, lives in water. That is a specification, not an animal, and you
    /// cannot picture it. This one is built from the same numbers but answers a
    /// different question, and it leans on touch, weight and movement rather
    /// than on colour, because those are the things that actually tell you what
    /// something is like.
    pub fn body(&self, gravity: f64, dim_light: bool) -> String {
        let t = &self.tr;
        let mut out = String::new();

        // How big, against something you have held.
        let size = if t[MULTI] < 0.25 {
            "far too small to see. A smear on glass, and there are millions of them"
        } else if t[SIZE] < 0.30 {
            "about the size of a grain of rice"
        } else if t[SIZE] < 0.45 {
            "about the size of a coin, and thicker than you would expect"
        } else if t[SIZE] < 0.60 {
            "roughly a handful. It would sit in cupped palms with room over"
        } else if t[SIZE] < 0.78 {
            "cat-sized, and heavier to lift than it looks"
        } else {
            "big enough that you would step back. Dog-sized, and dense with it"
        };
        out.push_str(&format!("It is {}.", size));

        // Build, which gravity decides more than anything else.
        if t[MULTI] > 0.4 {
            let build = if gravity > 1.4 {
                " Everything about it is low and braced, legs short and set wide, \
                  because standing up here costs something"
            } else if gravity < 0.7 {
                " It is long and spindly in a way that would not hold together \
                  on a heavier world"
            } else {
                " It is built squat and even, nothing about it exaggerated"
            };
            out.push_str(build);
            out.push('.');
        }

        // Surface: the thing you would notice with your hands.
        let skin = if self.land && t[DRY] > 0.6 {
            "Dry and slightly rough all over, like unglazed clay left in the sun"
        } else if self.land {
            "Faintly damp, and cooler than the air"
        } else if t[SIZE] > 0.6 {
            "Smooth and firm, with the give of a peeled boiled egg"
        } else {
            "Soft, and it flattens where you press and comes back slowly"
        };
        out.push_str(&format!(" {}.", skin));

        // Movement, which is most of what an animal is.
        let motion = if t[MOTIL] > 0.75 {
            if self.land { "It moves in short violent bursts and then holds \
                            absolutely still, and the stillness is more \
                            noticeable than the running" }
            else { "It goes through the water in one flick of the whole body, \
                    then coasts, then flicks again" }
        } else if t[MOTIL] > 0.45 {
            "It moves steadily and without hurry, and stops the moment it is \
             looked at directly"
        } else if t[MOTIL] > 0.2 {
            "It shifts position over minutes rather than seconds. You notice it \
             has moved rather than seeing it move"
        } else {
            "It does not move at all, and after a while that stops being strange"
        };
        out.push_str(&format!(" {}.", motion));

        // Senses. In dim red light, eyes are a poor investment.
        if t[SENSE] > 0.35 {
            let sense = if dim_light {
                "It has no eyes worth the name. What it has instead are fine \
                 feelers along its length that read the water, or the air, for \
                 movement, and they are always going"
            } else if t[SENSE] > 0.7 {
                "It has several eyes, set well apart, and they track separately"
            } else {
                "It has eyes, small and set low, and it uses them less than it \
                 uses whatever the feelers are doing"
            };
            out.push_str(&format!(" {}.", sense));
        }

        // Feeding apparatus, only if it is doing something specific.
        if self.is_predator() && t[SIZE] > 0.4 {
            out.push_str(" The front end opens wider than seems reasonable.");
        } else if t[PHOTO] > 0.55 {
            out.push_str(" Most of it is spread thin and flat, turned toward \
                          whatever light there is.");
        }

        if t[MANIP] > 0.45 {
            out.push_str(" It has something at the front it can hold things with, \
                          and it does, constantly, whether or not there is a reason.");
        }
        if t[SYMB] > 0.65 {
            out.push_str(" Something smaller lives on it, and neither of them \
                          appears to mind.");
        }
        if t[SOCIAL] > 0.5 {
            out.push_str(" You will never see just one.");
        }
        out
    }
}
