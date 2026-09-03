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

pub fn prompt(bio: &Biosphere, p: &Planet, star: &Star, t: f64, last: &str) -> Step {
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
            "huh" | "what" | "eh" | "wat" => huh(bio, last, &arg),
            "help" | "?" => help(),
            _ => println!("I do not know \"{}\". Type help for the list.", cmd),
        }
    }
}

fn help() {
    println!("
  huh              what just happened, in small words
  huh oxygen       what any word means, in small words
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

/// "huh" - what just happened, or what a word means, with nothing assumed.
///
/// Written for somebody about five or six, and written properly rather than
/// written down to. Short sentences, concrete things, no word that needs
/// another word to understand it. Done well this reads fine at any age, which
/// matters here: whoever is at the keyboard should get a real answer.
///
/// The event lookup matches on the headline text. That is only safe because
/// this program wrote those headlines itself, so they are known strings and
/// not something arriving from outside.
fn huh(bio: &Biosphere, last: &str, arg: &str) {
    println!();
    if !arg.is_empty() {
        match glossary(&arg.to_lowercase()) {
            Some(text) => println!("{}", text),
            None => println!(
                "I do not have a small-words answer for \"{}\" yet. Try: huh oxygen, \
                 huh star, huh gene, huh evolution.", arg),
        }
        return;
    }

    let l = last.to_lowercase();
    let says = |k: &str| l.contains(k);

    let text = if says("copies itself") {
        "Something in the water made a copy of itself.\n  \
         That is all being alive is, to begin with: something that can make \
         another one of itself.\n  \
         The copy came out a bit wrong. That turns out to matter more than \
         anything else that ever happens."
    } else if says("energy directly out of sunlight") {
        "Something learned to eat light.\n  \
         Before this, living things had to eat chemicals leaking out of hot \
         rock at the bottom of the sea, and there is only so much of that.\n  \
         Light falls on everywhere, all day, and never runs out."
    } else if says("split water") {
        "Something learned to pull water apart to get at what is inside it.\n  \
         Water is everywhere, so this works very well, and the thing that \
         figured it out takes over the sea.\n  \
         It leaves a gas behind. The gas is oxygen, and right now oxygen is \
         poison to everything alive."
    } else if says("oxygen goes into the air") || says("nothing left to absorb") {
        "The air has changed, and this is the biggest thing that has happened \
         so far.\n  \
         For about a billion years all the oxygen went straight into the sea \
         and got stuck there. Now the sea is full, so it goes up into the sky \
         and stays.\n  \
         Nearly everything alive is poisoned by it and dies.\n  \
         The few that survive get something enormous: you can burn food with \
         oxygen, and that gives about sixteen times more energy. Everything \
         big, and everything fast, and everything that ever thinks, comes from \
         this."
    } else if says("ozone") {
        "High above the ground, the oxygen has made a thin shield.\n  \
         Before it, sunlight burned anything that came out of the water.\n  \
         The land has been bare rock this whole time. It is not deadly any more."
    } else if says("stop separating") {
        "Cells started sticking together after they split, instead of drifting \
         apart.\n  \
         A clump is harder to eat than one cell on its own.\n  \
         Then the cells in the middle started doing different jobs from the \
         ones on the outside. That is a body."
    } else if says("swallows another cell") {
        "One cell ate another cell and did not digest it.\n  \
         They both just kept living, and kept splitting, together, until there \
         was no sensible way to say there were two of them.\n  \
         The eaten one was good at using oxygen. The one that ate it was big \
         and slow. Together they are better than either.\n  \
         This really happened, about two billion years ago, and it is why you \
         are made the way you are."
    } else if says("eating other living things") {
        "Something started eating other living things instead of making its \
         own food.\n  \
         Everything changes now. Being big helps. Being fast helps. Being \
         hidden helps. Being able to see what is coming helps.\n  \
         None of that was worth anything last week."
    } else if says("carrying signals") {
        "Some cells inside the body turned into wires.\n  \
         Now the body can feel something at one end and move at the other."
    } else if says("nervous system dense enough") {
        "Its brain got big enough to hold a picture of things that are not in \
         front of it.\n  \
         It can remember somewhere it is not standing. It can guess what \
         happens next.\n  \
         There is something it is like to be this animal now. There was not, \
         before."
    } else if says("groups that persist") {
        "They started living in groups that last longer than any one of them \
         does.\n  \
         So when one of them learns something, it does not die with them."
    } else if says("picks up something") {
        "One of them picked up something that was not part of its body, used \
         it to do a job its body could not do, and then kept it."
    } else if says("looks up") || says("works out what it is") {
        "Something on this world looked up and worked out what it was looking \
         at.\n  \
         It worked out that the metal inside its own blood was made in a star \
         that died before its sun existed.\n  \
         That is not a pretty way of putting it. That is where the metal came \
         from."
    } else if says("kilometres across") || says("kilometres of stone") || says("impact") {
        "A rock from space hit the world. A big one.\n  \
         The ground it landed on turned to dust and went up into the sky, and \
         the sky stayed dark for years.\n  \
         When the sky is dark, nothing can eat light, and then nothing can eat \
         the things that eat light."
    } else if says("mantle") || says("basalt") || says("lava") || says("seafloor spreading") {
        "The ground cracked open and hot rock came out, and kept coming out, \
         for hundreds of thousands of years.\n  \
         Nothing came from space. The world did this by itself.\n  \
         It changed the air faster than anything alive could keep up with."
    } else if says("ice") || says("freeze") || says("albedo") {
        "The whole world froze, all the way to the middle where it is warmest.\n  \
         White ice bounces sunlight back into space, which makes it colder, \
         which makes more ice, which bounces more sunlight away.\n  \
         Once it starts it does not stop by itself. Volcanoes have to breathe \
         enough warm air back in to break it, and that takes a very long time."
    } else if says("light years") && says("collapses") || says("second sun") {
        "A star near this one ran out and exploded.\n  \
         For a few weeks there were two suns in the sky.\n  \
         Then the hard part of the light arrived and stripped the shield off \
         the top of the air."
    } else if says("lineages alive the year before are gone") {
        "A lot of things just died at once.\n  \
         The ones that live through it are usually small, and not fussy about \
         food, and able to wait.\n  \
         Then they spread out into all the empty space the dead ones left, and \
         turn into something that looks nothing like what was here before. \
         The interesting things usually happen right after the terrible things."
    } else if says("most abundant thing alive") {
        "The commonest living thing on the world is not the same one as it was.\n  \
         Something else does the job better now, so there is more of it."
    } else if says("oxygen has climbed") || says("oxygen has fallen") {
        "The amount of oxygen in the air has changed.\n  \
         Living things are what put it there, and other living things are what \
         take it away, so this number is really a fight between them."
    } else if says("lineages has risen") {
        "There are more different kinds of living thing than there were.\n  \
         When something new becomes possible, everything spreads out into it \
         at once."
    } else if says("surface has gone from") {
        "The world got warmer or colder.\n  \
         What decides it is mostly how much of a certain gas is in the air, \
         and living things keep changing how much of it there is."
    } else if says("nothing about this world changes") {
        "Nothing happened for a very long time.\n  \
         Most of the history of a living world is like this. It is not broken. \
         It is just quiet."
    } else {
        "Something changed on this world.\n  \
         Try \"world\" to see what it is like right now, or \"life\" to see \
         what is living on it."
    };
    println!("{}", indent2(text));
    println!();
    println!("{}", indent2(&where_you_are(bio)));
}

fn where_you_are(bio: &Biosphere) -> String {
    let e = &bio.env;
    let n = bio.species.len();
    let air = if e.o2 < 0.001 { "There is no air you could breathe." }
        else if e.o2 < 0.05 { "There is a little bit of the air you breathe, not enough." }
        else { "There is air you could breathe." };
    let warm = if e.t_surf > 305.0 { "It is too hot." }
        else if e.t_surf > 283.0 { "It is warm." }
        else if e.t_surf > 268.0 { "It is cold." }
        else { "It is frozen." };
    let big = bio.species.iter().map(|s| s.tr[SIZE]).fold(0.0, f64::max);
    let size = if big < 0.3 { "Everything alive is too small to see." }
        else if big < 0.55 { "The biggest things alive would fit on your hand." }
        else { "Some of the things alive are as big as a dog." };
    let land = if e.land_open { "Things can live on the land." }
        else { "Nothing can live on the land yet." };
    format!("Right now: {} {} {} {} There are {} different kinds of living thing.",
        air, warm, size, land, n)
}

fn indent2(s: &str) -> String {
    s.lines().map(|l| format!("  {}", l.trim_start())).collect::<Vec<_>>().join("\n")
}

/// Plain meanings for the words the program uses. Same rule as above: real
/// answers, small words, nothing that needs a second word to understand it.
fn glossary(word: &str) -> Option<&'static str> {
    let w = word.trim().trim_start_matches("a ").trim_start_matches("the ");
    Some(match w {
        "star" | "sun" | "stars" =>
            "  A star is a ball of gas so big and so heavy that its own weight \
             squeezes the middle until it gets hot enough to burn.\n  \
             It burns for a very long time and then it stops. Our sun is a star.",
        "planet" | "planets" | "world" =>
            "  A planet is a ball of rock or gas going round and round a star.\n  \
             It is made of the dust left over from making the star.",
        "moon" | "moons" =>
            "  A moon is a smaller ball going round and round a planet.\n  \
             Ours got made when something enormous hit the Earth and knocked a \
             piece off.",
        "oxygen" =>
            "  Oxygen is the part of the air you breathe that keeps you alive.\n  \
             There was none of it at first. Living things made it, by accident, \
             as rubbish they did not want.\n  \
             It is also good at breaking things, which is why it poisoned \
             almost everything the first time it appeared.",
        "air" | "atmosphere" =>
            "  The air is the layer of gas sitting on top of a world.\n  \
             It keeps the heat in, like a blanket, and it stops some of the \
             burny kind of sunlight getting to the ground.",
        "carbon dioxide" | "co2" =>
            "  A gas that holds heat in. The more of it in the air, the warmer \
             the world.\n  \
             Volcanoes put it in. Rain slowly takes it out. Living things mess \
             with both.",
        "gene" | "genes" =>
            "  A gene is one instruction inside a living thing.\n  \
             All of them together are the whole recipe for making one.",
        "genome" =>
            "  The whole set of instructions for making one living thing.\n  \
             When something has a copy made of it, sometimes an instruction \
             comes out wrong, and that is where new kinds come from.",
        "mutation" | "mutate" =>
            "  A mistake in the copy.\n  \
             Nearly all of them are useless or bad. Every so often one is \
             good, and that one gets made again and again.",
        "evolution" =>
            "  Living things make copies of themselves, the copies come out a \
             bit different, and the ones that suit where they live end up \
             making more copies than the ones that do not.\n  \
             Do that for four billion years and you get everything.",
        "lineage" | "lineages" | "species" =>
            "  One kind of living thing, and everything descended from it.\n  \
             When one kind splits into two that no longer mix, that is two.",
        "photosynthesis" | "phototrophy" =>
            "  Eating light.\n  \
             Plants do it. It is where nearly all the food on Earth starts.",
        "predator" | "hunting" =>
            "  Something that eats other living things instead of making its \
             own food.",
        "ozone" =>
            "  A thin layer of oxygen high above the ground that soaks up the \
             burny kind of sunlight.\n  \
             Without it, nothing could live out of the water.",
        "extinction" | "extinct" =>
            "  When a kind of living thing has all died and there are no more \
             of it, ever.",
        "supernova" =>
            "  A very big star running out and blowing itself apart.\n  \
             It makes new kinds of stuff in the blast, and throws them out \
             into space. Some of that stuff is in you.",
        "metallicity" | "metals" =>
            "  How much of the stuff that is not hydrogen exists yet.\n  \
             At the start the universe only made the two lightest things. \
             Everything else - the iron, the oxygen, the carbon in you - got \
             made inside stars and let out when they died.",
        "redshift" =>
            "  A way of saying how long ago something was.\n  \
             Bigger number means longer ago.",
        "tectonics" | "plate tectonics" =>
            "  The outside of a world being broken into pieces that slowly move \
             around and bump into each other.\n  \
             It makes mountains and volcanoes, and it keeps a world from \
             getting stuck too hot or too cold.",
        "greenhouse" =>
            "  Gas in the air holding heat in, like a blanket on a bed.",
        "vents" | "hydrothermal" =>
            "  Cracks at the bottom of the sea where hot water comes out of the \
             rock.\n  \
             Life probably started at one of these, because there is energy \
             there you can use without needing the sun.",
        "biosphere" =>
            "  Everything alive on a world, all together, counted as one thing.",
        "cell" | "cells" =>
            "  The smallest piece of a living thing that is itself alive.\n  \
             You are made of a very large number of them. For most of the \
             history of life, everything alive was just one.",
        "symbiosis" | "endosymbiosis" =>
            "  Two different living things living together so closely that \
             neither works on its own any more.\n  \
             Sometimes they stop being two things at all.",
        "gravity" =>
            "  Everything pulls on everything else. Heavy things pull harder.\n  \
             It is what gathers dust into worlds and holds you on the ground.",
        "galaxy" =>
            "  A very large group of stars, all going round together.\n  \
             Ours has a few hundred billion in it.",
        "light year" | "lightyear" =>
            "  How far light gets in one year. Light is the fastest thing there \
             is, so this is a very long way.",
        "seed" =>
            "  The number you give it at the start.\n  \
             The same number always makes the same universe, exactly. If you \
             like one, write the number down.",
        _ => return None,
    })
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
