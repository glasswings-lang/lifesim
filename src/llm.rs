//! Optional live narration.
//!
//! The simulation computes facts. This module hands those facts to a language
//! model and asks it to write the passage, so that two runs of the same
//! universe are told differently even though they contain exactly the same
//! events. Without it, the program has a fixed vocabulary and starts to sound
//! like a form letter by the third billion years.
//!
//! Two rules govern what is sent and what comes back:
//!
//! 1. The model is given the computed facts and told to preserve them. It is
//!    never asked to decide what happened. If the model and the simulation ever
//!    disagree, the simulation is right, and the numbers printed under each
//!    passage come straight from the simulation regardless.
//! 2. Nothing personal is ever sent. The payload is invented cosmology.
//!
//! There are no HTTP crates here. Requests go out through `curl`, which every
//! machine this runs on already has, and the API key is passed in a config file
//! rather than on the command line so it never appears in a process listing.

use std::io::Write;
use std::process::Command;

#[derive(Clone, Copy, PartialEq)]
pub enum Backend {
    /// The built-in prose. Deterministic, offline, and eventually repetitive.
    Builtin,
    /// A model running locally, through Ollama. Private and free.
    Ollama,
    /// A hosted model, through OpenRouter. Needs OPENROUTER_API_KEY.
    OpenRouter,
}

/// Free models on OpenRouter, in the order they are tried. Chosen by running
/// real narration prompts through every free model the service offered, at the
/// batch size this program actually uses:
///
///   minimax-m3       6-23s, correct format, every computed number preserved
///                    across repeated trials, and visibly different wording
///                    each time. Reasoning must be turned off explicitly.
///                    Chosen.
///   glm-5.2          capable, but rate-limited on every attempt. Fallback.
///   gemma-4-31b      same. Second fallback.
///
/// Rejected: minimax-m2.7 writes well but its endpoint makes reasoning
/// mandatory, and on a full-sized batch it spent twenty-five thousand tokens
/// thinking and returned an empty message. nemotron-3-super answered with the
/// facts as a bulleted list rather than prose; nemotron-3.5-lightning and
/// nemotron-3-ultra ignored the output format; inkling is restricted to
/// agentic harnesses; dots-3 and openrouter/free returned nothing.
///
/// The lesson worth keeping: a two-passage test told the opposite story. These
/// were only separated by testing at the size the program really sends.
pub const FREE_MODELS: [&str; 3] = [
    "minimax/minimax-m3:free",
    "z-ai/glm-5.2:free",
    "google/gemma-4-31b-it:free",
];

pub struct Narrator {
    pub backend: Backend,
    pub model: String,
    /// Where Ollama is. Usually this machine, but it can be another one on the
    /// network - a laptop with more memory, reached over a private tunnel.
    pub ollama_host: String,
    /// Models to fall back to when the current one errors or rate-limits.
    pub fallbacks: Vec<String>,
    pub temperature: f64,
    /// Images and phrases already spent, so the model can be told to stop
    /// reaching for them. This is most of what keeps a long run from circling.
    pub used: Vec<String>,
    pub failures: u32,
    tmpdir: std::path::PathBuf,
}

impl Narrator {
    pub fn new(backend: Backend, model: Option<String>) -> Narrator {
        let asked = model.is_some();
        let model = model.unwrap_or_else(|| match backend {
            Backend::Ollama => "mistral:latest".into(),
            // Free by default. Nobody should discover they have been billed for
            // watching a universe because they forgot a flag.
            Backend::OpenRouter => FREE_MODELS[0].to_string(),
            Backend::Builtin => String::new(),
        });
        // Only chain through the free list when the caller did not name a
        // model. If someone asked for a specific one, respect that and fail.
        let fallbacks = if asked || backend != Backend::OpenRouter {
            Vec::new()
        } else {
            FREE_MODELS[1..].iter().map(|s| s.to_string()).collect()
        };
        Narrator {
            backend, model, fallbacks,
            ollama_host: default_ollama_host(),
            temperature: 0.95,
            used: Vec::new(), failures: 0,
            tmpdir: std::env::temp_dir(),
        }
    }

    pub fn label(&self) -> String {
        match self.backend {
            Backend::Builtin => "the built-in prose".into(),
            Backend::Ollama => {
                if self.ollama_host.contains("127.0.0.1") || self.ollama_host.contains("localhost") {
                    format!("{}, running locally through Ollama", self.model)
                } else {
                    format!("{}, through Ollama at {}", self.model, self.ollama_host)
                }
            }
            Backend::OpenRouter => format!("{}, through OpenRouter", self.model),
        }
    }

    /// Decide what is actually usable right now, without making the caller
    /// think about it. Preference order is: what was asked for, then a local
    /// model, then a hosted one, then the built-in prose.
    pub fn resolve_at(asked: Option<Backend>, model: Option<String>, host: Option<String>)
        -> Narrator
    {
        let have_key = std::env::var("OPENROUTER_API_KEY").map(|k| !k.trim().is_empty()).unwrap_or(false);
        let host = host.unwrap_or_else(default_ollama_host);
        let have_ollama = ollama_up_at(&host);
        let chosen = match asked {
            Some(Backend::OpenRouter) if have_key => Backend::OpenRouter,
            Some(Backend::OpenRouter) => {
                eprintln!("(No OPENROUTER_API_KEY is set, so the built-in prose is being used.)");
                Backend::Builtin
            }
            Some(Backend::Ollama) if have_ollama => Backend::Ollama,
            Some(Backend::Ollama) => {
                eprintln!("(Ollama is not answering at {}, so the built-in prose is being used.)", host);
                Backend::Builtin
            }
            Some(Backend::Builtin) => Backend::Builtin,
            None => {
                if have_key { Backend::OpenRouter }
                else if have_ollama { Backend::Ollama }
                else { Backend::Builtin }
            }
        };
        let mut n = Narrator::new(chosen, model);
        n.ollama_host = host;
        n
    }

    /// Ask for a batch of passages to be retold. Returns one string per input,
    /// or None if anything at all went wrong, in which case the caller keeps
    /// the built-in text and the run continues.
    pub fn retell(&mut self, chapter: &str, batch: &[Passage]) -> Option<Vec<String>> {
        if self.backend == Backend::Builtin || batch.is_empty() { return None; }
        if self.failures >= 3 { return None; }

        let prompt = build_prompt(chapter, batch, &self.used);
        let system = SYSTEM.to_string();

        // Free endpoints rate-limit without warning, so a failure moves down
        // the list rather than giving up on narration for the rest of the run.
        loop {
            let raw = match self.backend {
                Backend::Ollama => self.call_ollama(&system, &prompt),
                Backend::OpenRouter => self.call_openrouter(&system, &prompt),
                Backend::Builtin => None,
            };
            let ok = raw.as_ref()
                .map(|t| split_passages(t, batch.len()).len() == batch.len())
                .unwrap_or(false);
            if ok {
                let parts = split_passages(&raw.unwrap(), batch.len());
                for p in &parts { self.remember(p); }
                return Some(parts);
            }
            if self.fallbacks.is_empty() {
                self.failures += 1;
                return None;
            }
            let next = self.fallbacks.remove(0);
            eprintln!("(narrator: {} did not answer usefully; trying {}.)", self.model, next);
            self.model = next;
        }
    }

    fn remember(&mut self, text: &str) {
        for w in text.split(|c: char| !c.is_alphabetic()) {
            let w = w.to_lowercase();
            if w.len() > 6 && !COMMON.contains(&w.as_str()) && !self.used.contains(&w) {
                self.used.push(w);
            }
        }
        // Keep only what was used recently; the model does not need the whole
        // history, and an enormous avoid-list makes the prose stilted.
        let n = self.used.len();
        if n > 90 { self.used.drain(0..n - 90); }
    }

    fn call_ollama(&self, system: &str, prompt: &str) -> Option<String> {
        let body = format!(
            "{{\"model\":{},\"stream\":false,\"options\":{{\"temperature\":{}}},\
              \"messages\":[{{\"role\":\"system\",\"content\":{}}},\
                            {{\"role\":\"user\",\"content\":{}}}]}}",
            jstr(&self.model), self.temperature, jstr(system), jstr(prompt));
        let payload = self.write_temp("lifesim_payload.json", &body)?;
        let out = Command::new("curl")
            .arg("-sS").arg("--max-time").arg("240")
            .arg("-H").arg("Content-Type: application/json")
            .arg("-d").arg(format!("@{}", fwd(&payload)))
            .arg(format!("{}/api/chat", self.ollama_host.trim_end_matches('/')))
            .output().ok()?;
        let _ = std::fs::remove_file(&payload);
        self.check(&out.stderr);
        extract_content(&String::from_utf8_lossy(&out.stdout))
    }

    /// Say something the first time a request goes wrong. A narrator that
    /// quietly stops narrating and leaves you with the fallback prose, giving
    /// no reason, is worse than one that fails loudly.
    fn check(&self, stderr: &[u8]) {
        if stderr.is_empty() { return; }
        let msg = String::from_utf8_lossy(stderr);
        let msg = msg.trim();
        if !msg.is_empty() {
            eprintln!("(narrator: {})", msg.lines().next().unwrap_or(""));
        }
    }

    fn call_openrouter(&self, system: &str, prompt: &str) -> Option<String> {
        // Try with reasoning switched off. Several of the good free models are
        // reasoning models, and left to themselves they will spend the entire
        // token budget thinking and return an empty message. Some endpoints
        // refuse to have it disabled, so a refusal is retried without the flag.
        match self.post_openrouter(system, prompt, true) {
            Ok(text) => Some(text),
            Err(msg) => {
                if msg.to_lowercase().contains("reasoning") {
                    self.post_openrouter(system, prompt, false).ok()
                } else {
                    if !msg.is_empty() { eprintln!("(narrator: {})", trim(&msg, 140)); }
                    None
                }
            }
        }
    }

    fn post_openrouter(&self, system: &str, prompt: &str, no_reasoning: bool)
        -> Result<String, String>
    {
        let key = std::env::var("OPENROUTER_API_KEY").map_err(|_| "no api key".to_string())?;
        let reasoning = if no_reasoning { "\"reasoning\":{\"enabled\":false}," } else { "" };
        let body = format!(
            "{{\"model\":{},\"temperature\":{},{}\"max_tokens\":4000,              \"messages\":[{{\"role\":\"system\",\"content\":{}}},                            {{\"role\":\"user\",\"content\":{}}}]}}",
            jstr(&self.model), self.temperature, reasoning, jstr(system), jstr(prompt));
        let payload = self.write_temp("lifesim_payload.json", &body)
            .ok_or("could not write request")?;
        // The key goes in a config file, not in argv, so that it does not show
        // up for anyone running a process listing while this is in flight.
        // Built line by line on purpose. curl's config parser rejects a file
        // whose lines are indented, and a Rust string continuation quietly
        // leaves that indentation in, which produced a config that failed to
        // parse and an empty response with no explanation.
        let mut cfg = String::new();
        cfg.push_str("url = \"https://openrouter.ai/api/v1/chat/completions\"
");
        cfg.push_str(&format!("header = \"Authorization: Bearer {}\"
", key.trim()));
        cfg.push_str("header = \"Content-Type: application/json\"
");
        cfg.push_str(&format!("data = \"@{}\"
", fwd(&payload)));
        cfg.push_str("silent
");
        cfg.push_str("max-time = 240
");
        let cfgpath = self.write_temp("lifesim_curl.cfg", &cfg).ok_or("could not write config")?;
        let out = Command::new("curl").arg("--config").arg(fwd(&cfgpath)).output();
        let _ = std::fs::remove_file(&payload);
        let _ = std::fs::remove_file(&cfgpath);
        let out = out.map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if let Some(msg) = extract_error(&stdout) { return Err(msg); }
        extract_content(&stdout).ok_or_else(|| {
            // An empty message with a length finish is the signature of a
            // reasoning model that thought until it ran out of room.
            if stdout.contains("\"finish_reason\":\"length\"") {
                format!("{} used its whole budget on reasoning and returned nothing", self.model)
            } else if std::env::var("LIFESIM_DEBUG").is_ok() {
                format!("empty response; raw: {}", trim(&stdout, 400))
            } else {
                "empty response".to_string()
            }
        })
    }

    fn write_temp(&self, name: &str, contents: &str) -> Option<std::path::PathBuf> {
        let p = self.tmpdir.join(format!("{}_{}", std::process::id(), name));
        let mut f = std::fs::File::create(&p).ok()?;
        f.write_all(contents.as_bytes()).ok()?;
        Some(p)
    }
}

/// One thing that happened, with the numbers behind it.
pub struct Passage {
    pub kind: String,
    pub when: String,
    pub builtin: String,
    pub facts: Vec<(String, String)>,
}

const SYSTEM: &str = "\
You are the narrator of a physically simulated universe. A simulation has \
computed what happened; your only job is to tell it well.

Absolute rules:
- Never invent an event, object, creature or outcome that is not in the facts \
you are given. You are describing, not deciding.
- Every number, name and unit in the facts must appear in your prose, correct \
and unaltered. Do not round them differently or convert them.
- Do not add a moral, a lesson, or a message about humanity. Do not address \
the reader. Do not speculate about what comes next.
- Write plain paragraphs of prose. No headings, no bullet points, no markdown, \
no asterisks, no em-dash-heavy fragments, no rhetorical questions.

Voice: precise and unhurried, the way a very good science writer describes \
something enormous without straining for awe. Concrete over abstract. Prefer \
the specific mechanism to the grand summary. Let the scale speak for itself \
rather than telling the reader it is vast.

Vary your sentence lengths and openings. Do not begin consecutive passages the \
same way. If an AVOID list is given, treat those words and images as spent and \
reach for different ones.";

fn build_prompt(chapter: &str, batch: &[Passage], used: &[String]) -> String {
    let mut s = String::new();
    s.push_str(&format!("Chapter: {}\n\n", chapter));
    if !used.is_empty() {
        s.push_str("AVOID (already used in this run, find other words and images): ");
        s.push_str(&used.join(", "));
        s.push_str("\n\n");
    }
    s.push_str(&format!(
        "Retell each of the following {} passages. Output exactly {} passages, \
         each beginning with ### and its number on its own line, in order, and \
         nothing else. Keep each to one paragraph, roughly the length of the \
         draft given.\n\n",
        batch.len(), batch.len()));
    for (i, p) in batch.iter().enumerate() {
        s.push_str(&format!("### {}\n", i + 1));
        s.push_str(&format!("when: {}\n", p.when));
        s.push_str(&format!("event: {}\n", p.kind));
        if !p.facts.is_empty() {
            s.push_str("computed facts (all must survive into your prose):\n");
            for (k, v) in &p.facts {
                s.push_str(&format!("  - {}: {}\n", k, v));
            }
        }
        s.push_str(&format!("draft to replace: {}\n\n", p.builtin));
    }
    s
}

/// Split "### 1 ... ### 2 ..." back into passages.
fn split_passages(text: &str, want: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("###") {
            if started { out.push(cur.trim().to_string()); }
            cur = String::new();
            started = true;
            continue;
        }
        if started {
            // Models sometimes copy the labels out of the prompt back into the
            // answer. Drop anything that is plainly scaffolding rather than prose.
            let low = t.to_lowercase();
            if low.starts_with("when:") || low.starts_with("event:")
                || low.starts_with("computed facts") || low.starts_with("draft")
                || low.starts_with("passage") || low.starts_with("- ")
                || low.starts_with("chapter:") || low.starts_with("avoid")
                || t.is_empty()
            { continue; }
            cur.push_str(t);
            cur.push(' ');
        }
    }
    if started { out.push(cur.trim().to_string()); }
    // A stray label can also survive inline; strip a leading one if it did.
    for x in out.iter_mut() {
        for lead in ["when:", "event:", "draft to replace:", "computed facts:"] {
            let low = x.to_lowercase();
            if let Some(pos) = low.find(lead) {
                if pos < 3 {
                    let cut = pos + lead.len();
                    *x = x[cut..].trim_start().to_string();
                }
            }
        }
        // The stamp is printed in front of the passage by the caller, so a
        // model that also opens with "At 872.0 Myr, ..." produces it twice.
        // Drop that opening clause when it is clearly a restated timestamp.
        if x.starts_with("At ") || x.starts_with("By ") || x.starts_with("After ") {
            if let Some(comma) = x.find(", ") {
                if comma < 34 && x[..comma].chars().any(|c| c.is_ascii_digit()) {
                    let rest = x[comma + 2..].to_string();
                    let mut c = rest.chars();
                    if let Some(f) = c.next() {
                        *x = f.to_uppercase().collect::<String>() + c.as_str();
                    }
                }
            }
        }
        // "when: 3.0 minutes event: ..." leaves the stamp behind; take the prose
        // from the first capital letter that starts a sentence.
        let low = x.to_lowercase();
        if let Some(pos) = low.find("event:") {
            if pos < 40 { *x = x[pos + 6..].trim_start().to_string(); }
        }
    }
    out.retain(|x| !x.is_empty());
    if out.len() > want { out.truncate(want); }
    out
}

fn trim(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { s.chars().take(n).collect() }
}

/// Pull an API error message out of a response, if there is one.
fn extract_error(body: &str) -> Option<String> {
    let i = body.find("\"error\"")?;
    let seg = &body[i..];
    let j = seg.find("\"message\"")?;
    let seg = &seg[j + 9..];
    let k = seg.find('"')?;
    let seg = &seg[k + 1..];
    let end = seg.find('"')?;
    Some(seg[..end].to_string())
}

fn fwd(p: &std::path::Path) -> String {
    p.display().to_string().replace(char::from(92u8), "/")
}

/// Where to look for Ollama. OLLAMA_HOST is respected so that a machine on a
/// private network - reached over Tailscale, say - can do the work.
pub fn default_ollama_host() -> String {
    match std::env::var("OLLAMA_HOST") {
        Ok(h) if !h.trim().is_empty() => {
            let h = h.trim().to_string();
            if h.starts_with("http") { h } else { format!("http://{}", h) }
        }
        _ => "http://127.0.0.1:11434".to_string(),
    }
}

/// Is there a model server answering there?
pub fn ollama_up_at(host: &str) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    let hostport = host.trim_start_matches("http://").trim_start_matches("https://")
        .trim_end_matches('/');
    let hostport = if hostport.contains(':') { hostport.to_string() }
                   else { format!("{}:11434", hostport) };
    // A machine across a tunnel is slower to answer than one on this desk.
    let addrs = match hostport.to_socket_addrs() { Ok(a) => a, Err(_) => return false };
    for a in addrs {
        if TcpStream::connect_timeout(&a, Duration::from_millis(2500)).is_ok() { return true; }
    }
    false
}

/// Encode a Rust string as a JSON string literal, quotes included.
fn jstr(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Pull the assistant text out of either response shape without a JSON parser.
/// Both Ollama and OpenRouter put it under a "content" key; we take the longest
/// such value, which is reliably the message rather than a role or a stub.
fn extract_content(body: &str) -> Option<String> {
    let mut best: Option<String> = None;
    let bytes: Vec<char> = body.chars().collect();
    let pat: Vec<char> = "\"content\"".chars().collect();
    let mut i = 0;
    while i + pat.len() < bytes.len() {
        if bytes[i..i + pat.len()] == pat[..] {
            let mut j = i + pat.len();
            while j < bytes.len() && bytes[j] != '"' {
                if bytes[j] == '{' || bytes[j] == '[' { break; }
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '"' {
                j += 1;
                let mut val = String::new();
                while j < bytes.len() {
                    match bytes[j] {
                        '\\' if j + 1 < bytes.len() => {
                            match bytes[j + 1] {
                                'n' => val.push('\n'),
                                't' => val.push('\t'),
                                'r' => {}
                                'u' if j + 5 < bytes.len() => {
                                    let hex: String = bytes[j + 2..j + 6].iter().collect();
                                    if let Ok(n) = u32::from_str_radix(&hex, 16) {
                                        if let Some(ch) = char::from_u32(n) { val.push(ch); }
                                    }
                                    j += 4;
                                }
                                c => val.push(c),
                            }
                            j += 2;
                        }
                        '"' => break,
                        c => { val.push(c); j += 1; }
                    }
                }
                if best.as_ref().map(|b| val.len() > b.len()).unwrap_or(true) {
                    best = Some(val);
                }
            }
            i = j.max(i + 1);
        } else { i += 1; }
    }
    best.filter(|s| s.len() > 40)
}

const COMMON: [&str; 44] = [
    "because", "through", "between", "another", "against", "without", "already",
    "nothing", "everything", "something", "anything", "themselves", "himself",
    "which", "their", "there", "where", "while", "would", "could", "should",
    "before", "after", "about", "again", "still", "every", "other", "these",
    "those", "being", "having", "itself", "billion", "million", "thousand",
    "years", "planet", "system", "surface", "simulation", "universe", "world",
    "little",
];
