//! lifesim - one seed, one universe, told in words.
//!
//! Run it, and it goes: Planck instant -> inflation -> nucleosynthesis ->
//! recombination -> first stars -> the making of the elements -> a disk ->
//! planets -> climate -> chemistry -> life -> whatever life turns into here.
//!
//! Everything downstream depends on everything upstream. Change the seed and
//! you change how much iron the galaxy makes, which changes how big the planets
//! can be, which changes whether any of them holds an ocean, which changes
//! whether anything ever wakes up.

mod rng;
mod units;
mod narrate;
mod cosmos;
mod stars;
mod planets;
mod life;
mod llm;
mod toast;
mod explore;

use rng::Rng;
use units::*;
use narrate::{Scribe, Voice, Detail};
use planets::{Planet, Kind};
use stars::{Star, Abundances};
use life::*;
use llm::{Narrator, Backend};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        help();
        return;
    }
    match args[0].as_str() {
        "run" | "watch" | "tour" | "explore" => {
            let watching = args[0] == "watch" || args[0] == "tour";
            match parse(&args[1..], watching) {
                Ok(mut cfg) => {
                    if args[0] == "explore" {
                        cfg.explore = true;
                        cfg.pace = 0;
                        // Not terse by default. Shortening the prose turned out
                        // to cost information, and information is the thing the
                        // reader came for. --terse is there if you want it.
                        // Offline prose unless asked otherwise: exploring means
                        // stopping constantly, and waiting on a model at every
                        // stop would be slow and would spend tokens on passages
                        // nobody asked to read.
                        if cfg.narrator.is_none() { cfg.narrator = Some(Backend::Builtin); }
                    }
                    run(cfg)
                }
                Err(e) => { eprintln!("{}", e); eprintln!("Try: lifesim help"); }
            }
        }
        "guide" => guide(),
        "help" | "--help" | "-h" => help(),
        other => {
            eprintln!("I do not know the command \"{}\".", other);
            help();
        }
    }
}

struct Config {
    seed: u64,
    voice: Voice,
    detail: Detail,
    pace: u64,
    log: Option<String>,
    persist: bool,
    narrator: Option<Backend>,
    model: Option<String>,
    toast: bool,
    ollama_host: Option<String>,
    explore: bool,
    terse: bool,
}

fn parse(args: &[String], watching: bool) -> Result<Config, String> {
    let mut cfg = Config {
        seed: seed_from_clock(),
        voice: Voice::Lyric,
        detail: Detail::Normal,
        pace: if watching { 700 } else { 0 },
        log: None,
        persist: false,
        narrator: None,
        model: None,
        toast: false,
        ollama_host: None,
        explore: false,
        terse: false,
    };
    let mut i = 0;
    while i < args.len() {
        let need = |i: usize| -> Result<String, String> {
            args.get(i + 1).cloned().ok_or(format!("{} needs a value after it.", args[i]))
        };
        match args[i].as_str() {
            "--seed" => {
                let v = need(i)?;
                cfg.seed = v.parse::<u64>().map_err(|_| {
                    // A word is a fine seed too. Hash it.
                    String::new()
                }).unwrap_or_else(|_| hash_str(&v));
                i += 2;
            }
            "--voice" => {
                cfg.voice = match need(i)?.as_str() {
                    "plain" => Voice::Plain,
                    "lyric" => Voice::Lyric,
                    o => return Err(format!("voice must be plain or lyric, not \"{}\".", o)),
                };
                i += 2;
            }
            "--detail" => {
                cfg.detail = match need(i)?.as_str() {
                    "brief" => Detail::Brief,
                    "normal" => Detail::Normal,
                    "deep" => Detail::Deep,
                    o => return Err(format!("detail must be brief, normal or deep, not \"{}\".", o)),
                };
                i += 2;
            }
            "--pace" => {
                cfg.pace = need(i)?.parse().map_err(|_| "pace must be a number of milliseconds.".to_string())?;
                i += 2;
            }
            "--narrator" => {
                cfg.narrator = Some(match need(i)?.as_str() {
                    "builtin" | "off" => Backend::Builtin,
                    "ollama" | "local" => Backend::Ollama,
                    "openrouter" | "remote" => Backend::OpenRouter,
                    o => return Err(format!(
                        "narrator must be builtin, ollama or openrouter, not \"{}\".", o)),
                });
                i += 2;
            }
            "--model" => { cfg.model = Some(need(i)?); i += 2; }
            "--ollama-host" => { cfg.ollama_host = Some(need(i)?); i += 2; }
            "--log" => { cfg.log = Some(need(i)?); i += 2; }
            "--persist" => { cfg.persist = true; i += 1; }
            "--toast" => { cfg.toast = true; i += 1; }
            "--terse" => { cfg.terse = true; i += 1; }
            o => return Err(format!("I do not know the option \"{}\".", o)),
        }
    }
    Ok(cfg)
}

fn seed_from_clock() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ================================================================== the run ==

fn run(cfg: Config) {
    let narrator = Narrator::resolve_at(cfg.narrator, cfg.model.clone(),
                                        cfg.ollama_host.clone());
    let live = narrator.backend != Backend::Builtin;
    let label = narrator.label();
    let mut s = Scribe::new(cfg.voice, cfg.detail, cfg.pace, narrator);
    s.toaster = toast::Toaster::new(cfg.toast);
    s.terse = cfg.terse;
    if let Some(path) = &cfg.log {
        match std::fs::File::create(path) {
            Ok(f) => s.log = Some(f),
            Err(e) => eprintln!("(Could not open the log file: {})", e),
        }
    }
    let mut rng = Rng::new(cfg.seed);

    s.raw(&format!("Universe {}.", cfg.seed));
    s.raw("");
    for l in narrate::wrap("Everything below was computed, not chosen. Nothing         here is written down in advance.", 78, 0) { s.raw(&l); }
    if live {
        s.raw("");
        for l in narrate::wrap(&format!(
            "Narrated by {}. The physics is fixed by the seed and is identical every              time; the telling is not, so this run will read differently from the              last one even where the same things happen.", label), 78, 0) {
            s.raw(&l);
        }
    }

    // ---- Act I: cosmology ----
    let cos = cosmos::birth(cfg.seed, &mut rng);
    cosmos::tell_first_act(&cos, &mut s);
    if !cos.viable {
        closing(&mut s, cfg.seed, None);
        return;
    }

    // ---- Act II: stars and the elements ----
    let mut gal = stars::Galaxy::new(1.0e11, cos.y_helium);
    gal.calibrate(&mut rng);
    let history = stars::tell_second_act(&mut gal, &mut s, &mut rng, cos.t_first_yr);

    // ---- Act III: pick a star, and see what it built ----
    // We survey a sample of stars formed at various times in this galaxy's
    // history and follow the one whose system got furthest. That is a search,
    // not a cheat: the systems are all really built, and most of them are
    // disappointing.
    let mut best: Option<(f64, Star, Vec<Planet>, usize, f64)> = None;
    let survey = 18;
    for k in 0..survey {
        let mut r2 = rng.fork(k as u64);
        // A star's birth chemistry is the galaxy's chemistry at that moment.
        let idx = r2.int(4, history.len() as i64) as usize;
        let (t_form, ab): &(f64, Abundances) = &history[idx.min(history.len() - 1)];
        let mass = stars::sample_imf(&mut r2, 0.12, 3.0);
        let st = stars::make_star(mass, ab, *t_form);
        let sys = planets::form_system(&st, ab.rock_ratio(), &mut r2);
        // Score every world in it for the chance of anything happening.
        for (pi, _) in sys.iter().enumerate() {
            let mut sys2 = sys.clone();
            let age = st.life_myr * 0.05;
            for p in sys2.iter_mut() { planets::set_climate(p, &st, age, &mut r2); }
            let p = &sys2[pi];
            let sc = score(p, &st);
            if sc > best.as_ref().map(|b| b.0).unwrap_or(-1.0) {
                best = Some((sc, st.clone(), sys2.clone(), pi, *t_form));
            }
        }
    }

    let Some((score_v, star, mut sys, home_i, t_form)) = best else {
        s.chapter("III. Nothing to Stand On");
        s.say("Eighteen stars were surveyed across this galaxy's history and not \
               one of them assembled a solid body. There is too little heavy \
               material here. The universe works; it simply never gets around to \
               making anywhere to be.");
        closing(&mut s, cfg.seed, None);
        return;
    };

    let star_name = format!("{} — a {} star born {} after the first light",
        coin_name(&mut rng), stars::spectral_class(star.teff),
        years(t_form * 1e6));

    // Tidal locking is decided now, since it depends on the final orbits.
    for p in sys.iter_mut() {
        if planets::lock_time_myr(p, star.mass) < star.life_myr * 0.1 {
            p.tidally_locked = true;
            p.day_hours = p.period_days(star.mass) * 24.0;
        }
    }
    planets::tell_third_act(&star, &sys, &mut s, &star_name);

    let home = sys[home_i].clone();
    s.blank();
    s.say(&format!(
        "Of these, one is worth watching. Planet {}: {} Earth masses, {} of \
         surface gravity, {} Earth oceans of water, a surface at {}. It has no \
         name yet because nothing on it can name things.",
        home.name, format!("{:.2}", home.mass), format!("{:.2} g", home.gravity()),
        format!("{:.2}", home.water), temp(home.t_surf)));
    s.fact(Detail::Deep, "habitability score", &format!("{:.3}", score_v));

    // ---- Act IV: life ----
    let mut life_rng = rng.fork(0xB10);
    let outcome = fourth_act(&mut s, &mut life_rng, &star, &home, cfg.persist, cfg.explore);
    closing(&mut s, cfg.seed, Some(outcome));
}

/// How promising is this world? Used only to choose which world to follow.
fn score(p: &Planet, st: &Star) -> f64 {
    if !matches!(p.kind, Kind::Rocky | Kind::Ocean) { return -1.0; }
    let mut v = 0.0;
    let t = p.t_surf;
    v += (-((t - 288.0) / 45.0f64).powi(2)).exp() * 3.0;
    v += (p.water.min(5.0) / 5.0) * 1.2;
    if p.tectonics { v += 0.8; }
    if p.magnetic_field { v += 0.4; }
    if p.big_moon { v += 0.2; }
    v += (-(((p.mass - 1.0) / 1.6f64).powi(2))).exp() * 0.8;
    if p.pressure < 0.02 { v -= 2.0; }
    if p.tidally_locked { v -= 0.5; }
    // A star that dies in 200 million years is not going to see anything happen.
    v += (st.life_myr / 5000.0).min(1.5);
    v
}

// ------------------------------------------------------------ the long haul --

/// What the world looked like the last time anything was said about it.
struct Watch {
    div: usize,
    o2: f64,
    temp: f64,
    max_size: f64,
    land_share: f64,
    dom_id: u64,
    dom_name: String,
    dom_since: f64,
    last_report: f64,
    /// The last disaster described, so the same wording is not reached for
    /// twice running. Different eruptions should not read like reruns.
    last_shock: String,
}

struct Outcome {
    pub life: bool,
    pub peak_species: usize,
    pub reached: Reached,
    pub last_line: String,
}

fn fourth_act(
    s: &mut Scribe, rng: &mut Rng, star: &Star, home: &Planet, persist: bool,
    explore: bool,
) -> Outcome {
    s.chapter(&format!("IV. Planet {}, and What Happened On It", home.name));

    let mut p = home.clone();
    let env = new_env(&p, star, 0.0);
    let (rate, missing) = abiogenesis_odds(&p, &env);

    let mut out = Outcome {
        life: false, peak_species: 0, reached: Reached::default(),
        last_line: String::new(),
    };

    if rate <= 0.0 {
        s.say(&format!(
            "This world will never begin. {}. The chemistry required is not \
             merely unlikely here; it is unavailable. The planet turns for \
             billions of years and the only thing that ever changes is the \
             colour of the sky.",
            missing.join("; ")));
        out.last_line = "Sterile. The preconditions were never met.".into();
        return out;
    }
    if !missing.is_empty() {
        s.say(&format!("The odds are poor: {}. But poor is not zero, and there \
                        is a great deal of time.", missing.join("; ")));
    }

    s.say("The oceans are warm and the sky is carbon dioxide and nitrogen with \
           no free oxygen in it at all. Ultraviolet reaches the surface \
           unimpeded. At the sea floor, where water meets hot rock, there are \
           steep chemical gradients — protons on one side of a mineral membrane \
           and not the other — and a gradient is a battery. Molecules that \
           happen to catch that energy get to do more chemistry than molecules \
           that do not.");

    // --- waiting for the first replicator ---
    let mut t = 0.0f64;
    let star_life = star.life_myr;
    let horizon = star_life.min(12_000.0);
    let mut started = false;
    while t < horizon.min(2500.0) {
        t += 1.0;
        if rng.chance(rate) { started = true; break; }
    }
    if !started {
        s.beat(t * 1e6, "Nothing ever catches. There were candidate molecules, \
                         and cycles that ran for a while, and structures that \
                         held together for a season, but none of them ever made \
                         a copy of itself that could also make a copy. The world \
                         stays chemistry.");
        out.last_line = "Chemistry, but never biology.".into();
        return out;
    }

    let mut bio = Biosphere {
        species: Vec::new(), archive: Vec::new(), next_id: 1, env,
        total_biomass: 1.0, myr: t, rust: 8.0,
    };
    let mut first = Species {
        id: 0, genome: Genome::seed(rng), tr: [0.0; N_CH], rw: [0.0; N_CH],
        share: 1.0, born: t, parent: 0, name: coin_name(rng), land: false,
    };
    first.rw = first.genome.raw();
    first.tr = express(&first.rw, bio.env.o2);
    bio.record(&first);
    bio.species.push(first);
    out.life = true;
    out.reached.life = true;

    s.beat(t * 1e6, &format!(
        "Something copies itself. Not well — the copy is wrong perhaps one time \
         in ten — but wrongness is the point, because a copy that is slightly \
         different is a thing that can be selected. The first lineage is called \
         {} here, for the sake of having something to call it. It is a single \
         cell with {} genes, it eats the gradient at the vents, and oxygen would \
         kill it instantly if any existed.",
        bio.species[0].name, bio.species[0].genome.genes.len()));
    s.say("From this point nothing needs to be explained again. Everything that \
           follows is this, repeated, under changing conditions.");

    // --- the long simulation ---
    let mut impacts_left = 1.0f64;
    let mut peak = 1usize;
    let mut cont_phase = rng.range(0.0, 6.28);
    let mut snowball_cooldown = 0.0f64;
    let mut watch = Watch {
        div: 1, o2: 0.0, temp: bio.env.t_surf, max_size: 0.0, land_share: 0.0,
        dom_id: 0, dom_name: bio.species[0].name.clone(), dom_since: t,
        last_report: 0.0, last_shock: String::new(),
    };

    let stop_at = if persist { horizon } else { horizon.min(t + 6000.0) };
    let mut exploring = explore;
    let mut told = s.chronicle.len();
    let mut skip_until = 0.0f64;
    if exploring {
        s.flush();
        s.raw("");
        s.raw("You are on the surface of a world where something has just started copying itself.");
        s.raw("It will now run. Press enter to go to the next thing that happens, or type help.");
    }

    while t < stop_at {
        t += 1.0;
        life::step(&mut bio, rng, &p, star);
        if bio.species.is_empty() {
            s.beat(t * 1e6, "And then nothing. The last lineage thins out and \
                             stops. The planet is not damaged and the oceans are \
                             not gone; life simply lost, to the arithmetic, and \
                             there is no second start because the conditions that \
                             allowed the first one are used up.");
            out.last_line = "Life began and then ended.".into();
            return out;
        }
        peak = peak.max(bio.species.len());

        // --- the star ages, and the world warms ---
        if (t as i64) % 25 == 0 {
            let frac = (t / star_life).clamp(0.0, 1.0);
            let lum = star.lum * (1.0 + 0.6 * frac);
            p.t_eq = 278.5 * ((lum / (p.a * p.a)) * 0.7 / 0.7).powf(0.25);
            bio.env.light = (lum / (p.a * p.a)).min(4.0);
        }

        // --- continents drift ---
        // A supercontinent cycle of a few hundred million years opens and closes
        // seaways, isolating populations and then throwing them together.
        cont_phase += 0.0157;
        bio.env.land_frac = (0.28 + 0.16 * cont_phase.sin()).clamp(0.05, 0.5);

        // --- catastrophe ---
        impacts_left = (impacts_left * 0.9994).max(0.02);
        let mut shock: Option<(f64, String)> = None;
        if rng.chance(0.0055 * impacts_left) {
            let sev = rng.power_law(-2.0, 0.05, 0.95);
            let km = 2.0 + sev * 18.0;
            let dark = if out.reached.photo { " and photosynthesis stops" }
                       else { ", though nothing here yet lives on light" };
            let variants = [
                format!("A body roughly {:.0} kilometres across arrives at twenty                     kilometres a second. The impact vaporises rock, throws it up                     through the atmosphere, and it comes back down as a rain of                     molten glass that sets the continents alight. The dust closes                     the sky for a decade{}.", km, dark),
                format!("Something {:.0} kilometres wide, which has been on this                     orbit since before there were oceans, finally intersects the                     planet. The crater is still forming when the ejecta curtain                     reaches the far hemisphere. Rock dust and sulphate stay aloft                     for years{}.", km, dark),
                format!("An impact. {:.0} kilometres of stone and iron, delivering                     in about four seconds more energy than the planet's whole                     interior gives off in a century. The shock boils the shallow                     sea it lands in and the steam carries salt into the                     stratosphere{}.", km, dark),
                format!("A {:.0}-kilometre body strikes at a shallow angle, which                     is worse than a direct hit: it skips, and throws a longer                     plume further around the curve of the world. Ejecta reenters                     across a whole hemisphere and the ground glows dull red from                     horizon to horizon{}.", km, dark),
            ];
            shock = Some((sev, rng.pick(&variants).clone()));
        } else if rng.chance(0.0009) {
            let sev = rng.range(0.15, 0.7);
            let variants = [
                "A province of the mantle rises and a million cubic kilometres of                  basalt comes out over half a million years, along with enough                  sulphur to acidify the oceans and enough carbon dioxide to cook                  them afterward. Nothing arrives from space. The planet does this                  to itself.".to_string(),
                "A plume of hot mantle reaches the underside of the crust and                  stays there. The eruptions are not explosive and not dramatic to                  watch; they are simply continuous, for four hundred thousand                  years, and they put more carbon into the air than every volcano                  of the preceding hundred million.".to_string(),
                "Lava breaks out along a rift and keeps breaking out. What kills                  is not the flow but what the flow walks through: the magma                  intrudes into buried carbon and sulphur and bakes it out of the                  ground, and the atmosphere changes composition faster than                  anything living in it can track.".to_string(),
                "The seafloor spreading rate jumps and stays high. More ridge                  volcanism, more carbon dioxide, warmer oceans, and warm water                  holds less dissolved oxygen. The deep sea goes stagnant and                  anoxic, and stays that way long enough to matter.".to_string(),
            ];
            shock = Some((sev, rng.pick(&variants).clone()));
        } else if rng.chance(0.0004) {
            let sev = rng.range(0.1, 0.5);
            let variants = [
                "A star inside thirty light years reaches the end of its fuel and                  collapses. For a few weeks the sky has a second sun in it, and                  then the hard radiation arrives and strips the ozone away, and                  for three hundred years the ultraviolet reaches the ground                  unfiltered.".to_string(),
                "A nearby massive star finishes. The light is the harmless part                  and it is spectacular; what matters is the cosmic ray flux that                  follows months later, which breaks apart the upper atmosphere                  and leaves a haze of nitrogen oxides that shades the surface and                  falls as acid.".to_string(),
                "Something detonates close enough that the shielding fails. The                  shallow ocean, which has been the safest place on this world,                  stops being safe: ultraviolet penetrates the top few metres,                  which is exactly where most of the life is.".to_string(),
            ];
            shock = Some((sev, rng.pick(&variants).clone()));
        } else if bio.env.t_surf < 262.0 && t > snowball_cooldown && rng.chance(0.004) {
            snowball_cooldown = t + 400.0;
            let sev = rng.range(0.3, 0.8);
            let variants = [
                "Ice reaches the tropics. White ground reflects sunlight, which                  cools the ground, which makes more white ground, and the feedback                  runs all the way to the equator. The oceans freeze over to a                  depth of a kilometre and stay frozen while volcanoes slowly                  rebuild an atmosphere underneath.".to_string(),
                "The ice sheets pass the latitude where the albedo feedback                  becomes self-sustaining, and after that no further cooling is                  needed. The whole ocean skins over. Beneath the ice, in the dark,                  at the vents, everything that survives this survives there.".to_string(),
                "A second freeze, and this one closes the last open water.                  Weathering stops, because weathering needs rain and there is no                  longer any rain, so the carbon dioxide the volcanoes emit has                  nowhere to go and simply accumulates. It will take tens of                  millions of years, and then the thaw will be violent.".to_string(),
            ];
            shock = Some((sev, rng.pick(&variants).clone()));
        }

        // Reroll once if the wording would repeat the previous disaster.
        if let Some((sev, txt)) = shock.clone() {
            if txt == watch.last_shock { shock = None; }
            let _ = sev;
        }
        if shock.is_none() { }

        if let Some((sev, text)) = shock {
            watch.last_shock = text.clone();
            let before = bio.species.len();
            // Large-bodied, specialised, high-metabolism lineages die first.
            // Small things that can wait die last. This is what the fossil
            // record actually shows, and it falls out of the trait weighting
            // rather than being imposed.
            let mut survivors: Vec<Species> = Vec::new();
            for sp in bio.species.iter() {
                let exposure = 0.25 + 0.5 * sp.tr[SIZE] + 0.3 * sp.tr[NEURAL]
                    - 0.25 * sp.tr[DRY] - 0.2 * sp.tr[FECUND];
                let death = (sev * exposure * 1.6).clamp(0.0, 0.985);
                if !rng.chance(death) {
                    let mut q = sp.clone();
                    q.share *= 1.0 - death * 0.8;
                    survivors.push(q);
                }
            }
            if survivors.is_empty() {
                if let Some(one) = bio.species.first() { survivors.push(one.clone()); }
            }
            bio.species = survivors;
            let tot: f64 = bio.species.iter().map(|x| x.share).sum();
            if tot > 0.0 { for x in bio.species.iter_mut() { x.share /= tot; } }
            let lost = 1.0 - bio.species.len() as f64 / before.max(1) as f64;
            if lost > 0.25 {
                s.beat(t * 1e6, &text);
                s.say(&format!(
                    "{} of the lineages alive the year before are gone. What is \
                     left is small, patient and unspecialised, and it inherits an \
                     empty world. Within twenty million years the survivors have \
                     spread into every job the dead used to do, and they look \
                     nothing like what they replaced.",
                    pct(lost)));
                out.reached.life = true;
            }
        }

        // --- milestones, detected rather than scheduled ---
        report_milestones(s, &mut bio, &mut out.reached, t, &p);

        // --- pause and let somebody look around ---
        if exploring {
            // Passages are buffered and only reach the chronicle when they are
            // printed, so the flush has to come first. Without it the count
            // never moves and the prompt appears exactly once, which is what it
            // did before this line existed.
            s.flush();
            if s.chronicle.len() > told && t >= skip_until {
                let last = s.chronicle.last().map(|b| b.headline.clone())
                    .unwrap_or_default();
                match explore::prompt(&bio, &p, star, t, &last) {
                    explore::Step::Go => {}
                    explore::Step::Advance(n) => { skip_until = t + n; }
                    explore::Step::Release => { exploring = false; }
                    explore::Step::Quit => {
                        out.peak_species = peak;
                        out.last_line = "Left early, while it was still going.".into();
                        return out;
                    }
                }
            }
            told = s.chronicle.len();
        }

        // --- the state of the world, reported only when it changes ---
        //
        // The earlier version of this printed a status line every seven hundred
        // million years whether or not anything had happened, which is how a
        // four-billion-year middle turns into the same paragraph eight times.
        // What follows reports differences, and when there are none it keeps
        // count of the silence and says how long it lasted, once, at the end
        // of it. Long stretches of nothing are a real feature of this world's
        // history and they deserve one honest sentence, not eight identical ones.
        if (t as i64) % 25 == 0 && s.detail >= Detail::Normal
            && t - watch.last_report > 200.0 {
            let mut changes: Vec<String> = Vec::new();
            let div = bio.species.len();
            let o2 = bio.env.o2;
            let tc = bio.env.t_surf;
            let land_share: f64 = bio.species.iter().filter(|x| x.land).map(|x| x.share).sum();
            let big = bio.species.iter().filter(|x| x.share > 0.01)
                .map(|x| x.tr[SIZE]).fold(0.0, f64::max);
            let dom = dominant(&bio).map(|x| (x.id, x.name.clone(), x.share, x.describe()));

            if div as f64 > watch.div as f64 * 2.5 && div > 12 {
                changes.push(format!("the number of distinct lineages has risen from {} to {}",
                    watch.div, div));
            } else if (div as f64) < watch.div as f64 * 0.40 && watch.div > 12 {
                changes.push(format!("diversity has fallen from {} lineages to {}",
                    watch.div, div));
            }
            if o2 > watch.o2 * 3.0 && o2 > 1e-3 {
                changes.push(format!("oxygen has climbed from {} of the air to {}",
                    pct(watch.o2), pct(o2)));
            } else if o2 < watch.o2 * 0.4 && watch.o2 > 1e-3 {
                changes.push(format!("oxygen has fallen from {} of the air to {}",
                    pct(watch.o2), pct(o2)));
            }
            if (tc - watch.temp).abs() > 18.0 {
                changes.push(format!("the surface has gone from {} to {}",
                    temp(watch.temp), temp(tc)));
            }
            if big > watch.max_size + 0.12 {
                changes.push("the largest bodies on the planet are larger than                               anything that has lived here before".into());
            }
            if land_share > 0.05 && watch.land_share < 0.05 {
                changes.push("something is living on the land and staying there".into());
            }
            if let Some((id, ref name, share, ref desc)) = dom {
                if id != watch.dom_id && share > 0.25 && t - watch.dom_since > 400.0 {
                    changes.push(format!(
                        "the most abundant thing alive is no longer {} but {}, which is {}",
                        watch.dom_name, name, desc));
                    watch.dom_id = id;
                    watch.dom_name = name.clone();
                    watch.dom_since = t;
                }
            }

            if !changes.is_empty() {
                let quiet = t - watch.last_report;
                let preface = if quiet > 600.0 && watch.last_report > 0.0 {
                    format!("For {} nothing about this world changes in any way                              worth recording. Then: ", years(quiet * 1e6))
                } else { String::new() };
                let mut body = changes.join("; ");
                if preface.is_empty() {
                    let mut c = body.chars();
                    if let Some(f) = c.next() {
                        body = f.to_uppercase().collect::<String>() + c.as_str();
                    }
                }
                s.pulse(t * 1e6, "a change in the state of the whole world",
                    &format!("{}{}. There {}, the air is {} oxygen at {:.3} bar                               of carbon dioxide, and the surface sits at {}.",
                        preface, body,
                        if div == 1 { "is one lineage left".to_string() }
                        else { format!("are {} lineages", div) },
                        pct(o2), bio.env.co2, temp(tc)));
                watch.last_report = t;
                watch.div = div;
                watch.o2 = o2;
                watch.temp = tc;
                watch.max_size = watch.max_size.max(big);
                watch.land_share = land_share;
            }
        }
    }

    out.peak_species = peak;

    // --- the ending the star chooses ---
    s.chapter("V. The End of the Main Sequence");
    if t >= star_life * 0.99 {
        s.beat(t * 1e6, &format!(
            "The star runs out of hydrogen in its core. It swells, cools at the \
             surface and brightens enormously, and the habitable zone sweeps \
             outward past this planet and keeps going. The oceans boil off in a \
             few million years. Whatever is here at the end has {} to notice it \
             coming.",
            if out.reached.minds { "a mind capable of understanding what it is seeing" }
            else { "no way to know" }));
    } else {
        s.beat(t * 1e6, "The simulation reaches its horizon. The star is still \
                         burning and the world is still turning. This is not an \
                         ending, only the edge of what was computed. Run it again \
                         with --persist and it will keep going to the end of the \
                         star.");
    }

    survey_life(s, &bio, &out.reached);
    out.last_line = final_line(&out.reached);
    out
}

fn dominant(bio: &Biosphere) -> Option<&Species> {
    bio.species.iter().max_by(|a, b| a.share.partial_cmp(&b.share).unwrap())
}

/// Watch for thresholds being crossed. Nothing here causes anything; it only
/// notices, the way a person watching would notice.
fn report_milestones(
    s: &mut Scribe, bio: &mut Biosphere, r: &mut Reached, t: f64, p: &Planet,
) {
    let agg = |ch: usize, bio: &Biosphere| -> f64 {
        bio.species.iter().map(|x| x.share * x.tr[ch]).sum()
    };
    // The best any *established* lineage can do. The share filter matters more
    // than it looks: in a biosphere of two hundred lineages there is nearly
    // always some marginal variant that has drifted across any given threshold
    // and is about to die out. That is not the same as the biosphere having
    // acquired the capability, and counting it would make every world produce
    // everything eventually.
    let best = |ch: usize, bio: &Biosphere| -> f64 {
        bio.species.iter()
            .filter(|x| x.share > 0.015)
            .map(|x| x.tr[ch])
            .fold(0.0, f64::max)
    };

    if !r.photo && best(PHOTO, bio) > 0.32 {
        r.photo = true;
        s.beat(t * 1e6, "A lineage stops waiting for the rock to feed it and \
                         starts taking energy directly out of sunlight. It is \
                         not efficient and it does not yet split water, but the \
                         supply is effectively infinite and it falls on \
                         everywhere at once instead of only on the vents. Life \
                         leaves the sea floor.");
    }
    if !r.oxygenic && r.photo && best(PHOTO, bio) > 0.60 && agg(PHOTO, bio) > 0.25 {
        r.oxygenic = true;
        s.beat(t * 1e6, "Something works out how to split water. Water is \
                         everywhere and it is a far better source of electrons \
                         than anything that came before, and the lineage that \
                         manages it inherits the ocean. There is a waste product. \
                         Oxygen is violently reactive and nothing alive can \
                         handle it, and for now it all goes straight into the \
                         iron dissolved in the sea, which binds it and settles. \
                         The air does not change at all, and will not for a \
                         billion years.");
    }
    if !r.great_oxidation && bio.env.o2 > 0.02 {
        r.great_oxidation = true;
        s.beat(t * 1e6, &format!(
            "The sea has nothing left to absorb it. Oxygen goes into \
             the air and stays there, and the air is now {} oxygen and climbing. \
             This is the worst thing that has ever happened to life on this \
             planet. Almost everything alive is anaerobic and oxygen tears its \
             chemistry apart. The methane that has been keeping the world warm is \
             oxidised out of the sky, and the temperature falls.",
            pct(bio.env.o2)));
        s.say("And it is also the best thing that has ever happened, for the \
               small number of lineages that survive it. Burning food with oxygen \
               releases roughly sixteen times more energy than fermenting it. \
               Everything expensive that life will ever do — moving fast, growing \
               large, thinking — is affordable only on this budget, and the \
               budget did not exist until life itself created it as a poison.");
    }
    if !r.endosymbiosis {
        if let Some(sp) = bio.species.iter().find(|x| x.share > 0.015
            && x.genome.genes.len() > 30 && x.tr[SYMB] > 0.5 && x.tr[AEROBIC] > 0.4) {
            r.endosymbiosis = true;
            let name = sp.name.clone();
            let n = sp.genome.genes.len();
            s.beat(t * 1e6, &format!(
                "A cell swallows another cell and does not digest it. The \
                 swallowed one is good at oxygen; the swallower is large and \
                 slow. Neither dies. They divide together, and keep dividing \
                 together, until there is no longer a sensible way to say there \
                 are two of them. The lineage is called {} and its genome has \
                 {} genes, which is more than anything on this planet has ever \
                 carried. It is a cell with a power plant inside it, and it can \
                 now afford to be a hundred times bigger than its ancestors.",
                name, n));
        }
    }
    if !r.multicell && best(MULTI, bio) > 0.5 {
        r.multicell = true;
        s.beat(t * 1e6, "Cells stop separating after they divide. At first it \
                         is only a clump, and the clump is harder to eat than a \
                         single cell is, which is reason enough. Then cells in \
                         the middle find themselves in a different situation \
                         from cells on the outside, and start doing different \
                         things, and once that happens there is a body.");
    }
    if !r.predation {
        if let Some(sp) = bio.species.iter().find(|x| x.is_predator() && x.share > 0.03) {
            r.predation = true;
            let name = sp.name.clone();
            s.beat(t * 1e6, &format!(
                "{} begins eating other living things instead of making its own \
                 food. This changes the rules for everything at once. Being large \
                 is now a defence. Being fast is now a defence. Being armoured, \
                 being hidden, being poisonous, being able to see what is coming \
                 — none of these were worth anything last epoch and all of them \
                 are worth everything now. Diversity does not increase gradually \
                 after this. It detonates.", name));
        }
    }
    if !r.land && bio.env.land_open {
        r.land = true;
        s.beat(t * 1e6, &format!(
            "Enough oxygen has accumulated that a layer of ozone forms high in \
             the atmosphere, and ultraviolet at the ground drops to {} of what it \
             was. The land, which has been sterile rock and dust for the entire \
             history of this world, stops being lethal. Within a few million \
             years there is something growing on it.",
            pct(bio.env.uv / 0.95)));
        s.say("Note what had to happen for this. Something had to learn to eat \
               light. It had to learn to split water, and poison the whole world \
               doing it. The poison had to accumulate for a billion years until \
               it reached the upper atmosphere and became a shield. Only then \
               could anything crawl out. The land was opened by a waste product.");
    }
    if !r.nerves && best(NEURAL, bio) > 0.45 {
        r.nerves = true;
        s.beat(t * 1e6, "Some cells in some bodies specialise in carrying \
                         signals, and the bodies that have them can respond to \
                         the world faster than the world changes. A net of them \
                         thickens at the end that usually goes first.");
    }
    if !r.minds && best(NEURAL, bio) > 0.72 {
        r.minds = true;
        if let Some(sp) = bio.species.iter().filter(|x| x.share > 0.015)
            .max_by(|a, b| a.tr[NEURAL].partial_cmp(&b.tr[NEURAL]).unwrap()) {
            let name = sp.name.clone();
            let d = sp.describe();
            s.beat(t * 1e6, &format!(
                "{} has a nervous system dense enough to hold a model of things \
                 that are not currently in front of it. It can predict. It can \
                 remember a place it is not standing in. Somewhere in that \
                 machinery there is now something it is like to be this animal, \
                 and there was not before, and no line in this simulation records \
                 exactly when it started. It is {}.", name, d));
        }
    }
    if !r.society && best(SOCIAL, bio) > 0.6 {
        r.society = true;
        s.beat(t * 1e6, "They begin to live in groups that persist longer than \
                         any individual in them. Information now outlives the \
                         animal that learned it, which means it can accumulate, \
                         which means for the first time on this planet something \
                         is inherited that is not written in a gene.");
    }
    if !r.tools && r.land && best(MANIP, bio) > 0.62 && best(NEURAL, bio) > 0.72 {
        r.tools = true;
        s.beat(t * 1e6, "One of them picks up something that is not part of its \
                         body and uses it to do something its body cannot. Then \
                         it keeps it.");
    }
    if !r.civilisation && r.tools && r.society && r.land
        && best(MANIP, bio) > 0.80 && best(SOCIAL, bio) > 0.70 {
        r.civilisation = true;
        s.beat(t * 1e6, &format!(
            "{} after a molecule first copied itself badly at a hydrothermal \
             vent, something on this planet looks up at {} and works out what it \
             is. It gets the mass roughly right. It gets the distance roughly \
             right. It works out that the light arriving has taken time to \
             arrive, and that the light of the further ones has taken so long \
             that some of them are no longer there.",
            years(t * 1e6),
            if p.moons > 0 { "the moon" } else { "the sky" }));
        s.say("It works out that the iron in its own blood was made inside a \
               star that died before this one was born, and that the phosphorus \
               in its genes came out of the same explosion, and that this is not \
               a metaphor but a supply chain. It is, as far as it can tell, the \
               universe having a look at itself.");
    }
}

fn survey_life(s: &mut Scribe, bio: &Biosphere, r: &Reached) {
    if bio.species.is_empty() { return; }
    s.blank();
    s.say(&format!("At the end there are {} lineages. A cross-section, since                     listing the largest few would only show the same successful                     thing six times:", bio.species.len()));

    // Pick out lineages that are notable for different reasons, so the survey
    // says something about the shape of the biosphere rather than just its peak.
    let pick = |f: &dyn Fn(&Species) -> f64| -> Option<&Species> {
        bio.species.iter()
            .filter(|x| f(x).is_finite())
            .max_by(|a, b| f(a).partial_cmp(&f(b)).unwrap())
    };
    let mut shown: Vec<u64> = Vec::new();
    let entries: Vec<(&str, Box<dyn Fn(&Species) -> f64>)> = vec![
        ("the most abundant", Box::new(|x: &Species| x.share)),
        ("what most of this world's food comes from",
            Box::new(|x: &Species| x.share * (x.tr[PHOTO] + x.tr[CHEMO]))),
        ("the largest", Box::new(|x: &Species| x.tr[SIZE])),
        ("the most complex", Box::new(|x: &Species| x.tr[NEURAL])),
        ("the oldest surviving lineage", Box::new(|x: &Species| -x.born)),
    ];
    for (label, f) in entries {
        if let Some(sp) = pick(&*f) {
            if shown.contains(&sp.id) { continue; }
            shown.push(sp.id);
            s.blank();
            s.say(&format!("{} — {} ({} of the biosphere, {} genes, first                             appeared at {})",
                sp.name, label, pct(sp.share), sp.genome.genes.len(),
                stamp(sp.born * 1e6)));
            s.say(&format!("  {}.", sp.describe()));
        }
    }

    // The shape of the food web, which says more than any single organism does.
    let only_make: f64 = bio.species.iter()
        .filter(|x| x.is_producer() && !x.is_predator()).map(|x| x.share).sum();
    let only_take: f64 = bio.species.iter()
        .filter(|x| x.is_predator() && !x.is_producer()).map(|x| x.share).sum();
    let both: f64 = bio.species.iter()
        .filter(|x| x.is_predator() && x.is_producer()).map(|x| x.share).sum();
    let land: f64 = bio.species.iter().filter(|x| x.land).map(|x| x.share).sum();
    s.blank();
    s.say(&format!(
        "The shape of the food web, by share of the biosphere: {} lives          entirely on light or on rock chemistry, {} entirely by catching other          organisms, and {} does both, which is the arrangement this world seems          to have settled on. {} of everything alive is out of the water.",
        pct(only_make), pct(only_take), pct(both), pct(land)));

    // Did the interesting thing survive its own success?
    if r.minds {
        let still = bio.species.iter().any(|x| x.tr[NEURAL] > 0.6 && x.share > 0.01);
        s.blank();
        if still {
            s.say("Whatever was thinking on this world is still thinking.");
        } else {
            s.say("Nothing here has a nervous system worth the name any more.                    The lineage that did is gone — not destroyed by anything                    dramatic, in the end, only outcompeted by something cheaper                    to run. Being clever is expensive, and the bill comes due                    every generation.");
        }
    }
}

fn final_line(r: &Reached) -> String {
    if r.civilisation { "A civilisation. Something here knows where it came from.".into() }
    else if r.tools { "Tool users, on the edge of it.".into() }
    else if r.minds { "Minds, but no hands and no fire.".into() }
    else if r.nerves { "Animals with nervous systems. No further.".into() }
    else if r.multicell { "Bodies, but nothing that thinks.".into() }
    else if r.great_oxidation { "An oxygen atmosphere, and microbes to breathe it.".into() }
    else if r.photo { "Photosynthesis, and a world of green slime.".into() }
    else if r.life { "Life, but it never got off the sea floor.".into() }
    else { "No life.".into() }
}

fn closing(s: &mut Scribe, seed: u64, out: Option<Outcome>) {
    s.chapter("The Chronicle");
    s.flush();
    let beats: Vec<(f64, String)> = s.chronicle.iter()
        .map(|b| (b.year, b.headline.clone())).collect();
    for (y, h) in beats.iter() {
        let line = format!("{:>12}   {}", stamp(*y), h);
        for l in narrate::wrap(&line, 78, 15) { s.raw(&l); }
    }
    s.blank();
    if let Some(o) = out {
        s.say(&o.last_line);
        if o.peak_species > 0 {
            s.say(&format!("Peak diversity: {} coexisting lineages.", o.peak_species));
        }
    }
    s.say(&format!("Universe {}. To see it again, exactly: lifesim run --seed {}",
        seed, seed));
    s.flush();
}

// ==================================================================== help ==

fn help() {
    println!("{}", r#"
lifesim - a universe, from the first instant to whatever is alive at the end.

You give it a number. It gives you a cosmos: how fast it expanded, what
elements got made and by which stars dying, what planets condensed out of
the leftovers, what the weather was like on them, and whether anything ever
started copying itself. Then it tells you what happened, in words.

The same number always gives the same universe. Different numbers give
genuinely different ones, and most of them are disappointing, which is the
honest result.

COMMANDS

  lifesim run              Run a universe as fast as the machine can.
  lifesim watch            The same, but paced, so you can read it as it goes.
  lifesim explore          Walk around inside it. The run stops each time
                           something happens and you can look at any creature,
                           follow it back to what it came from, ask what is in
                           the ocean, or check the sky, then let it carry on.
  lifesim guide            What all the terms mean, in plain language.
  lifesim help             This.

OPTIONS

  --seed 12345             Pick the universe. A word works too: --seed hearth
  --voice lyric            Full prose. The default.
  --voice plain            Short factual sentences instead.
  --detail brief           Only the big events.
  --detail normal          The default.
  --detail deep            Every number the simulation computed.
  --pace 700               Milliseconds to rest between paragraphs.
                           0 is as fast as possible. "watch" defaults to 700.
  --log run.txt            Also write everything to a file.
  --terse                  One sentence per event instead of a paragraph.
                           Explore mode uses this by default.
  --toast                  Raise a Windows notification when something
                           actually happens, so you can leave the run in the
                           background. Only real events, never status lines,
                           and at most one every four seconds.
  --narrator openrouter    Have a language model tell it. Needs the
                           OPENROUTER_API_KEY environment variable set.
  --narrator ollama        The same, using Ollama. Free, private, and only as
                           good as the model you point it at.
  --ollama-host URL        Where Ollama is, if not this machine. Works for a
                           box on a private network, e.g.
                           --ollama-host http://my-mac:11434
                           The OLLAMA_HOST environment variable is used if
                           this is not given.
  --narrator builtin       The built-in prose. Offline and deterministic.
  --model NAME             Which model. Defaults to minimax/minimax-m3:free
                           for openrouter (free, costs nothing) and
                           mistral:latest for ollama. Pass a paid id such as
                           anthropic/claude-sonnet-4.5 for better prose at a
                           few cents a run.
  --persist                Keep simulating life until the star actually dies,
                           rather than stopping after six billion years.

EXAMPLES

  lifesim watch --seed hearth
  lifesim run --seed 42 --detail deep --log universe42.txt
  lifesim run --voice plain --detail brief

WHO IS TELLING IT

  With no --narrator given, it uses a language model if one is available
  (OpenRouter if the key is set, otherwise a local Ollama), and the built-in
  prose if neither is. The simulation is identical either way. Only the
  wording changes, so the same seed narrated twice reads differently while
  every number stays the same.

  The model is given the computed facts and told to write them. It is never
  asked what happened. If it ever contradicts the simulation, the simulation
  is right, and --detail deep prints the raw numbers underneath so you can
  check.

There are no graphics and there never will be. Everything is lines of text,
in order, readable top to bottom.
"#);
}

fn guide() {
    println!("{}", r#"
A GUIDE, IN PLAIN LANGUAGE

You do not need any of this to run the thing. It is here for when a word goes
past and you want to know what it was doing.

THE NUMBER YOU GIVE IT (the seed)
  One number decides the whole universe. Not the story - the physics. It sets
  how fast this universe expands, how much helium it made in its first twenty
  minutes, how lumpy it started out. Everything else is consequence. The same
  number always rebuilds the same universe, exactly, so if you like one, write
  it down.

REDSHIFT
  A way of saying "how long ago" that astronomers prefer. Higher means older.
  Redshift 1100 is when the universe first became transparent. Redshift 0 is now.

METALLICITY
  Astronomers call every element heavier than helium a "metal", which is
  annoying but standard. Oxygen is a metal. Carbon is a metal. The early
  universe had none of them, because it only made hydrogen, helium and a trace
  of lithium. Everything else was manufactured inside stars and released when
  those stars died. Metallicity is how much of that manufacturing has happened
  so far. It matters because you cannot build a rocky planet - or a body - out
  of hydrogen.

THE INITIAL MASS FUNCTION
  When a cloud collapses into stars it does not make them all the same size.
  It makes a great many small ones and very few large ones, in a specific
  proportion that has been measured. Big stars are brilliant and die fast. Small
  ones are dim and last longer than the universe has existed so far.

THE SNOW LINE
  The distance from a star beyond which water is ice rather than vapour. Inside
  it, planets form dry and rocky. Outside, they form with several times more
  material, which is why the giant planets are always out there.

THE ISOLATION MASS
  A growing planet eats everything in the ring of orbit it can reach, and then
  it stops, because there is nothing left within reach. How big it gets before
  that happens is the isolation mass, and it depends on how much dust there was,
  which depends on the metallicity, which depends on how many stars have died.

TIDALLY LOCKED
  Close to a small star, a planet's rotation gets dragged into matching its
  orbit, so one side faces the star forever. Probably the most common situation
  for a temperate planet in this universe.

THE CARBONATE-SILICATE CYCLE
  A thermostat made of rock. Volcanoes put carbon dioxide into the air, which
  warms the planet. Rain pulls it back out, and rain does this faster when it is
  warmer. So the planet self-corrects, slowly - over hundreds of thousands of
  years - and can stay temperate even while its star gets brighter. It needs
  liquid water and moving continents. Without both, there is no thermostat.

WHAT A GENE IS, HERE
  A small thing that pushes a body in one direction, with a strength. A genome
  is a list of them. Mutation nudges the values, and sometimes copies a whole
  gene, and a copy is free to change into something new because the original is
  still doing the old job. That is where nearly every real innovation in the
  history of life came from.

WHY THE ORDER OF EVENTS IS ALWAYS ROUGHLY THE SAME
  It is not scripted. It is gated. A nervous system is enormously expensive to
  run, so nothing can afford one until there is a high-energy way to eat, which
  means oxygen, which does not exist until something has been making it as a
  waste product for a billion years. Multicellular bodies have the same problem.
  So the order comes out of the energy budget, not out of a plan - which is why
  runs where oxygen never accumulates stay microbial forever, and do.

MASS EXTINCTIONS
  Nothing chooses who dies. Large bodies, expensive brains and narrow diets are
  weighted to die; small, patient, undemanding things are weighted to live. Then
  the survivors inherit an empty world and radiate into it. This is why the
  interesting things usually happen right after the terrible things.

WHEN A RUN IS BORING
  Most are. A universe with too much dark energy never forms a galaxy. A galaxy
  that runs out of gas early never makes enough iron for rocks. A planet without
  plate tectonics has no thermostat and no chemistry to eat. A star that lives
  400 million years does not live long enough for anything to happen. That is
  the honest answer to how common life is, as far as anyone actually knows: the
  preconditions are many and they are all required.
"#);
}
