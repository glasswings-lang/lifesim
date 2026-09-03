//! Walking around inside a running world, instead of being read a summary of it.
//!
//! The simulation always computed all of this. Every lineage has a genome, a
//! parent, sixteen traits, a way of making a living and a date it appeared;
//! every world has its water, its air, its day length and its sky. The original
//! program worked all of that out and then printed twelve one-line
//! announcements and threw the rest away.
//!
//! This module keeps the door open. The run pauses when something happens, and
//! you can ask about anything present before letting it go on.
//!
//! Two things shape the writing here:
//!
//! * Short by default, long on request. Nothing volunteers three sentences
//!   where one will do. You ask for depth and you get it, and only then.
//! * Everything is a plain line of text and the prompt is a plain prompt. No
//!   redraw, no cursor movement, no colour. A screen reader reads this the same
//!   way it reads everything else.

use std::io::{self, BufRead, Write};
use crate::life::*;
use crate::planets::{Planet, kind_name};
use crate::stars::{Star, spectral_class, colour};
use crate::units::*;

pub enum Step {
    /// Carry on to the next thing that happens.
    Go,
    /// Carry on for a set number of million years, whatever happens.
    Advance(f64),
    /// Stop asking; run to the end.
    Release,
    Quit,
}

pub fn prompt(bio: &Biosphere, p: &Planet, star: &Star, t: f64) -> Step {
    let stdin = io::stdin();
    loop {
        print!("\n{} > ", stamp(t * 1e6));
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            return Step::Release;
        }
        let line = line.trim().to_string();
        let mut it = line.splitn(2, ' ');
        let cmd = it.next().unwrap_or("").to_lowercase();
        let arg = it.next().unwrap_or("").trim().to_string();

        match cmd.as_str() {
            "" | "go" | "next" | "n" => {
                if arg.is_empty() { return Step::Go; }
                match arg.trim_end_matches(|c: char| c.is_alphabetic()).trim().parse::<f64>() {
                    Ok(v) if v > 0.0 => {
                        let myr = if arg.to_lowercase().contains('g') { v * 1000.0 } else { v };
                        return Step::Advance(myr);
                    }
                    _ => println!("Say a number of million years, like: go 50"),
                }
            }
            "run" | "release" => return Step::Release,
            "q" | "quit" | "exit" => return Step::Quit,
            "look" | "l" => look(bio, &arg),
            "back" | "parent" => back(bio, &arg),
            "kin" | "children" => kin(bio, &arg),
            "life" | "who" | "alive" => who(bio, &arg),
            "ocean" | "sea" | "water" => where_(bio, false),
            "land" => where_(bio, true),
            "world" | "here" => world(bio, p),
            "sky" | "star" => sky(star, p),
            "help" | "?" => help(),
            _ => println!("I do not know \"{}\". Type help for the list.", cmd),
        }
    }
}

fn help() {
    println!("
  (enter)          let it run to the next thing that happens
  go 50            let 50 million years pass, whatever happens
  look NAME        everything known about one lineage
  back NAME        what it came from
  kin NAME         what came from it
  life             what is alive now, biggest first
  ocean / land     what lives where
  world            this planet right now
  sky              the star and the rest of the system
  run              stop asking and go to the end
  quit             leave
");
}

/// One lineage, in as much depth as exists.
fn look(bio: &Biosphere, name: &str) {
    if name.is_empty() { println!("Look at what? Try: look {}", any_name(bio)); return; }
    let Some(a) = bio.find(name) else {
        println!("Nothing here called \"{}\". Try: life", name);
        return;
    };
    println!();
    println!("{}", a.name);
    println!("  {}.", a.desc);
    match bio.alive(a.id) {
        Some(sp) => {
            println!("  Alive. {} of everything living.", pct(sp.share));
            println!("  Appeared {}, so it has lasted {}.",
                stamp(a.born * 1e6), years((bio.myr - a.born) * 1e6));
            println!("  Genome: {} genes.", sp.genome.genes.len());
            let t = &sp.tr;
            println!("  What it is good at:");
            let mut traits: Vec<(f64, &str)> = (0..N_CH)
                .map(|i| (t[i], CH_NAMES[i])).collect();
            traits.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
            for (v, n) in traits.iter().take(5) {
                println!("    {:<28} {}", n, bar_words(*v));
            }
            println!("  Weakest at: {} and {}.",
                traits[N_CH - 1].1, traits[N_CH - 2].1);
            match (sp.is_predator(), sp.is_producer()) {
                (true, true) => println!("  It hunts, and it also makes some of its own food. Plenty of single-celled things do both."),
                (true, false) => println!("  It hunts."),
                (false, true) => println!("  It makes its own food."),
                (false, false) => println!("  It scavenges what it can."),
            }
            let eaten_by: Vec<&str> = bio.species.iter()
                .filter(|q| q.is_predator() && q.tr[SIZE] > t[SIZE] * 0.75 && q.id != sp.id)
                .map(|q| q.name.as_str()).take(4).collect();
            if !eaten_by.is_empty() {
                println!("  Eaten by: {}.", eaten_by.join(", "));
            }
        }
        None => {
            let died = a.died.unwrap_or(bio.myr);
            println!("  Extinct. Lived from {} to {} - {}.",
                stamp(a.born * 1e6), stamp(died * 1e6),
                years((died - a.born) * 1e6));
            println!("  At its largest it was {} of the biosphere.", pct(a.peak));
            println!("  Genome: {} genes.", a.genes);
        }
    }
    if a.parent != a.id {
        if let Some(par) = bio.archive.iter().find(|x| x.id == a.parent) {
            println!("  Came from {}. (back {})", par.name, a.name);
        }
    }
    let n_kids = bio.archive.iter().filter(|x| x.parent == a.id).count();
    if n_kids > 0 {
        println!("  {} lineages have split off from it. (kin {})", n_kids, a.name);
    }
}

fn back(bio: &Biosphere, name: &str) {
    let Some(a) = bio.find(name) else { println!("No lineage called \"{}\".", name); return; };
    println!();
    println!("Walking back from {}:", a.name);
    // Only the rungs where something actually changed. Most steps in a lineage
    // are indistinguishable from their parent, and printing forty identical
    // lines hides the three places where the animal became a different animal.
    let mut chain: Vec<Archived> = Vec::new();
    let mut cur = a.clone();
    let mut depth = 0;
    loop {
        chain.push(cur.clone());
        if cur.parent == cur.id || depth > 400 { break; }
        match bio.archive.iter().find(|x| x.id == cur.parent) {
            Some(par) => { cur = par.clone(); depth += 1; }
            None => break,
        }
    }
    let total = chain.len();
    let mut shown = 0;
    let mut last = String::new();
    for (i, link) in chain.iter().enumerate() {
        let changed = link.desc != last;
        let endpoint = i == 0 || i == total - 1;
        if changed || endpoint {
            let state = if bio.alive(link.id).is_some() { "alive" } else { "extinct" };
            println!("  {:>12}  {} ({}) - {}",
                stamp(link.born * 1e6), link.name, state, link.desc);
            last = link.desc.clone();
            shown += 1;
        }
    }
    println!("  {} generations of lineage, {} of them different enough to show.",
        total, shown);
    if total > 1 {
        println!("  (and before all of it, the first thing that copied itself)");
    }
}

fn kin(bio: &Biosphere, name: &str) {
    let Some(a) = bio.find(name) else { println!("No lineage called \"{}\".", name); return; };
    let kids: Vec<&Archived> = bio.archive.iter().filter(|x| x.parent == a.id).collect();
    println!();
    if kids.is_empty() { println!("Nothing ever split off from {}.", a.name); return; }
    println!("{} lineages split off from {}:", kids.len(), a.name);
    for k in kids.iter().take(14) {
        let state = if bio.alive(k.id).is_some() { "alive" } else { "extinct" };
        println!("  {:>12}  {} ({}) - {}", stamp(k.born * 1e6), k.name, state, k.desc);
    }
    if kids.len() > 14 { println!("  ...and {} more.", kids.len() - 14); }
}

fn who(bio: &Biosphere, arg: &str) {
    let n: usize = arg.parse().unwrap_or(10);
    let mut v: Vec<&Species> = bio.species.iter().collect();
    v.sort_by(|a, b| b.share.partial_cmp(&a.share).unwrap());
    println!();
    println!("{} lineages alive. The largest {}:", bio.species.len(), n.min(v.len()));
    for sp in v.iter().take(n) {
        println!("  {:>7}  {:<14} {}", pct(sp.share), sp.name, sp.describe());
    }
    println!("(look NAME for any of them)");
}

fn where_(bio: &Biosphere, land: bool) {
    let mut v: Vec<&Species> = bio.species.iter().filter(|s| s.land == land).collect();
    v.sort_by(|a, b| b.share.partial_cmp(&a.share).unwrap());
    println!();
    if v.is_empty() {
        println!("Nothing lives {} yet.", if land { "on land" } else { "in the water" });
        return;
    }
    let share: f64 = v.iter().map(|s| s.share).sum();
    println!("{} lineages live {}, {} of everything alive:",
        v.len(), if land { "on land" } else { "in the water" }, pct(share));
    for sp in v.iter().take(8) {
        println!("  {:>7}  {:<14} {}", pct(sp.share), sp.name, sp.describe());
    }
}

fn world(bio: &Biosphere, p: &Planet) {
    let e = &bio.env;
    println!();
    println!("Planet {}, at {}", p.name, stamp(bio.myr * 1e6));
    println!("  air              {} oxygen, {:.3} bar of carbon dioxide", pct(e.o2), e.co2);
    println!("  surface          {}", temp(e.t_surf));
    println!("  sunlight         {:.2} times what Earth gets", e.light);
    println!("  ultraviolet      {} of what it was before there was life", pct(e.uv / 0.95));
    println!("  land             {} of the surface, and it is {}",
        pct(e.land_frac), if e.land_open { "survivable" } else { "still sterile" });
    println!("  water            {:.2} Earth oceans", p.water);
    println!("  gravity          {:.2} g", p.gravity());
    println!("  day              {:.1} hours", p.day_hours);
    println!("  tilt             {:.0} degrees", p.obliquity);
    println!("  moons            {}", p.moons);
    println!("  living things    {} lineages", bio.species.len());
}

fn sky(star: &Star, p: &Planet) {
    println!();
    println!("The star");
    println!("  mass             {:.2} suns", star.mass);
    println!("  brightness       {:.3} suns", star.lum);
    println!("  surface          {}", temp(star.teff));
    println!("  colour           {}", colour(star.teff));
    println!("  type             {}", spectral_class(star.teff));
    println!("  lifetime         {}", years(star.life_myr * 1e6));
    println!();
    println!("From this planet");
    println!("  distance         {:.3} AU", p.a);
    println!("  year             {:.0} days", p.period_days(star.mass));
    println!("  this world is    {}", kind_name(p.kind));
}

fn bar_words(v: f64) -> &'static str {
    if v > 0.85 { "as good as it gets here" }
    else if v > 0.7 { "very" }
    else if v > 0.5 { "quite" }
    else if v > 0.3 { "a little" }
    else if v > 0.12 { "barely" }
    else { "not at all" }
}

fn any_name(bio: &Biosphere) -> String {
    bio.species.first().map(|s| s.name.clone()).unwrap_or_else(|| "something".into())
}
