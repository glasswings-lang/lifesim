//! The voice of the simulation.
//!
//! Everything the reader learns arrives through here. Three rules govern this
//! module and they are not negotiable:
//!
//! 1. Output is plain lines of text. No progress bars, no spinners, no cursor
//!    tricks, no box-drawing, no colour as the sole carrier of meaning. A
//!    screen reader must be able to read this top to bottom and lose nothing.
//! 2. The numbers come from the physics. The prose only ever describes what the
//!    simulation actually computed. We never invent an event for flavour.
//! 3. The *wording* may vary and the *facts* may not. When a language model is
//!    narrating, it is given the computed facts and asked to write them well.
//!    It is never asked what happened.
//!
//! Passages are buffered a chapter at a time rather than printed immediately,
//! because retelling them in one batch lets the narrator see what it has
//! already said and stop reaching for the same images. That batching is the
//! main defence against a long run turning into a form letter.

use std::io::Write;
use crate::units;
use crate::llm::{Narrator, Passage, Backend};
use crate::toast::Toaster;

#[derive(Clone, Copy, PartialEq)]
pub enum Voice {
    /// Full prose. The default, because a universe deserves it.
    Lyric,
    /// Short declarative sentences. For when you want the facts fast.
    Plain,
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Detail {
    Brief = 0,
    Normal = 1,
    Deep = 2,
}

/// One thing that happened, kept for the closing chronicle.
pub struct Beat {
    pub year: f64,
    pub headline: String,
}

enum Slot {
    Chapter(String),
    Prose {
        kind: String,
        year: Option<f64>,
        text: String,
        facts: Vec<(String, String)>,
        indent: usize,
        chronicle: bool,
    },
    Fact(Detail, String, String),
    Aside(Detail, String),
    Blank,
}

pub struct Scribe {
    pub voice: Voice,
    pub detail: Detail,
    /// Milliseconds to rest between paragraphs, so a run can be *watched*
    /// rather than scrolled past. Zero means as fast as the machine goes.
    pub pace_ms: u64,
    pub width: usize,
    pub chronicle: Vec<Beat>,
    pub log: Option<std::fs::File>,
    pub year: f64,
    narrator: Option<Narrator>,
    pending: Vec<Slot>,
    chapter_title: String,
    pub toaster: Toaster,
}

impl Scribe {
    pub fn new(voice: Voice, detail: Detail, pace_ms: u64, narrator: Narrator) -> Self {
        Scribe {
            voice, detail, pace_ms,
            width: 78,
            chronicle: Vec::new(),
            log: None,
            year: 0.0,
            narrator: Some(narrator),
            pending: Vec::new(),
            chapter_title: "Opening".into(),
            toaster: Toaster::new(false),
        }
    }

    #[allow(dead_code)]
    pub fn narrator_label(&self) -> String {
        self.narrator.as_ref().map(|n| n.label()).unwrap_or_default()
    }

    pub fn narrator_is_live(&self) -> bool {
        self.narrator.as_ref().map(|n| n.backend != Backend::Builtin).unwrap_or(false)
    }

    // ------------------------------------------------------------ recording --

    /// A section heading. Flushes everything buffered before it, so each
    /// chapter is retold as a unit.
    pub fn chapter(&mut self, title: &str) {
        self.flush();
        self.chapter_title = title.to_string();
        self.pending.push(Slot::Chapter(title.to_string()));
    }

    /// A timestamped moment.
    pub fn beat(&mut self, year: f64, text: &str) {
        self.year = year;
        self.pending.push(Slot::Prose {
            kind: "a moment in the history".into(),
            year: Some(year),
            text: strip(text),
            facts: Vec::new(),
            indent: 4,
            chronicle: true,
        });
    }

    /// A timestamped moment that is routine bookkeeping rather than an event,
    /// and so does not belong in the closing chronicle.
    pub fn pulse(&mut self, year: f64, kind: &str, text: &str) {
        self.year = year;
        self.pending.push(Slot::Prose {
            kind: kind.into(),
            year: Some(year),
            text: strip(text),
            facts: Vec::new(),
            indent: 4,
            chronicle: false,
        });
    }

    /// A listed item: a planet, a species. No timestamp, no chronicle entry.
    pub fn item(&mut self, text: &str) {
        self.pending.push(Slot::Prose {
            kind: "one object in a list".into(),
            year: None,
            text: strip(text),
            facts: Vec::new(),
            indent: 4,
            chronicle: false,
        });
    }

    /// Prose with no timestamp: description, atmosphere, consequence.
    pub fn say(&mut self, text: &str) {
        self.pending.push(Slot::Prose {
            kind: "continuing description".into(),
            year: None,
            text: strip(text),
            facts: Vec::new(),
            indent: 0,
            chronicle: false,
        });
    }

    /// A labelled fact. Attaches to the passage above it, so a live narrator
    /// gets the numbers and can work them into the sentence.
    pub fn fact(&mut self, level: Detail, label: &str, value: &str) {
        if let Some(Slot::Prose { facts, .. }) = self.pending.iter_mut().rev()
            .find(|s| matches!(s, Slot::Prose { .. }))
        {
            facts.push((label.to_string(), value.to_string()));
        }
        self.pending.push(Slot::Fact(level, label.to_string(), value.to_string()));
    }

    /// Detail that only appears at higher --detail levels.
    pub fn aside(&mut self, level: Detail, text: &str) {
        self.pending.push(Slot::Aside(level, strip(text)));
    }

    pub fn blank(&mut self) { self.pending.push(Slot::Blank); }

    pub fn phrase(&self, lyric: &str, plain: &str) -> String {
        match self.voice { Voice::Lyric => lyric.into(), Voice::Plain => plain.into() }
    }

    // -------------------------------------------------------------- emitting --

    /// Retell what is buffered, then print it.
    pub fn flush(&mut self) {
        if self.pending.is_empty() { return; }
        let slots = std::mem::take(&mut self.pending);
        let slots = self.retell(slots);
        for slot in slots { self.emit_slot(slot); }
    }

    fn retell(&mut self, mut slots: Vec<Slot>) -> Vec<Slot> {
        let mut nar = match self.narrator.take() { Some(n) => n, None => return slots };
        if nar.backend == Backend::Builtin || self.voice == Voice::Plain {
            self.narrator = Some(nar);
            return slots;
        }
        // Which slots are prose, in order.
        //
        // List items - the planet-by-planet inventory - are deliberately left
        // alone. They are already terse lines of pure fact, they gain little
        // from being rewritten, and there can be twenty of them in a system,
        // which on a rate-limited free endpoint is the difference between a
        // run that finishes and one that does not.
        let idx: Vec<usize> = slots.iter().enumerate()
            .filter(|(_, s)| matches!(s, Slot::Prose { kind, .. }
                if kind != "one object in a list"))
            .map(|(i, _)| i).collect();

        // A live narrator means a network round trip before anything can be
        // printed, which is a silent gap of several seconds. Say so, once, in
        // one static line. Not a spinner: a sentence that a screen reader can
        // read and then move past.
        if !idx.is_empty() {
            let out = std::io::stdout();
            let mut h = out.lock();
            let _ = writeln!(h, "
(Composing: {}. A few seconds.)", self.chapter_title);
            let _ = h.flush();
        }

        for chunk in idx.chunks(10) {
            let batch: Vec<Passage> = chunk.iter().map(|&i| {
                match &slots[i] {
                    Slot::Prose { kind, year, text, facts, .. } => Passage {
                        kind: kind.clone(),
                        when: year.map(units::stamp).unwrap_or_else(|| "no particular time".into()),
                        builtin: text.clone(),
                        facts: facts.clone(),
                    },
                    _ => unreachable!(),
                }
            }).collect();
            if let Some(retold) = nar.retell(&self.chapter_title, &batch) {
                for (k, &i) in chunk.iter().enumerate() {
                    if let Slot::Prose { text, .. } = &mut slots[i] {
                        *text = retold[k].clone();
                    }
                }
            }
        }
        self.narrator = Some(nar);
        slots
    }

    fn emit_slot(&mut self, slot: Slot) {
        // When a model is doing the narration it is handed the facts and works
        // them into the sentence, so printing them again underneath is just the
        // same numbers twice. At --detail deep they stay, as the audit trail.
        let live = self.narrator_is_live();
        match slot {
            Slot::Chapter(t) => {
                self.line("");
                self.line(&t);
                let rule = "-".repeat(t.chars().count().min(self.width));
                self.line(&rule);
                self.rest(1.5);
            }
            Slot::Prose { year, text, indent, chronicle, .. } => {
                let body = match year {
                    Some(y) => format!("[{}]  {}", units::stamp(y), text),
                    None => text.clone(),
                };
                if indent > 0 { self.line(""); }
                for l in wrap(&body, self.width, indent) { self.line(&l); }
                if chronicle {
                    if let Some(y) = year {
                        let head = first_sentence(&text);
                        // Only real events reach the notification tray. Status
                        // lines and list items are deliberately excluded.
                        self.toaster.notify(
                            &format!("lifesim - {}", units::stamp(y)), &head);
                        self.chronicle.push(Beat { year: y, headline: head });
                    }
                }
                self.rest(1.0);
            }
            Slot::Fact(level, label, value) => {
                let show = if live { Detail::Deep } else { level };
                if self.detail >= show {
                    self.line(&format!("  {}: {}", label, value));
                }
            }
            Slot::Aside(level, text) => {
                if self.detail >= level {
                    for l in wrap(&text, self.width, 2) { self.line(&l); }
                    self.rest(0.4);
                }
            }
            Slot::Blank => self.line(""),
        }
    }

    fn line(&mut self, s: &str) {
        let out = std::io::stdout();
        let mut h = out.lock();
        let _ = writeln!(h, "{}", s);
        let _ = h.flush();
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{}", s);
        }
    }

    /// Print immediately, bypassing the buffer. For the closing chronicle,
    /// which is assembled from passages that have already been told.
    pub fn raw(&mut self, s: &str) { self.line(s); }

    fn rest(&self, factor: f64) {
        if self.pace_ms > 0 {
            let ms = (self.pace_ms as f64 * factor) as u64;
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
    }
}

/// Wrap text to a width, indenting continuation lines. Hard-wrapping keeps
/// terminal output stable and keeps a screen reader's line navigation sane.
pub fn wrap(text: &str, width: usize, indent: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let extra = if line.is_empty() { 0 } else { 1 };
        let cur = line.chars().count() + if out.is_empty() { 0 } else { indent };
        if cur + extra + word.chars().count() > width && !line.is_empty() {
            out.push(if out.is_empty() { line.clone() } else { format!("{}{}", pad, line) });
            line.clear();
            line.push_str(word);
        } else {
            if !line.is_empty() { line.push(' '); }
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        out.push(if out.is_empty() { line } else { format!("{}{}", pad, line) });
    }
    if out.is_empty() { out.push(String::new()); }
    out
}

fn strip(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first sentence, for the chronicle. Retold passages vary in length, so
/// the chronicle takes the opening claim rather than a fixed character count.
fn first_sentence(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '.' || c == '?' {
            if matches!(chars.peek(), Some(' ') | None) && out.chars().count() > 24 { break; }
        }
        if out.chars().count() > 150 { out.push_str("..."); break; }
    }
    out.trim().to_string()
}
