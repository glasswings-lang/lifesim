# lifesim

A universe, from the first instant to whatever is alive at the end, told in words.

You give it a number. It builds a whole cosmos out of that number — how fast it
expands, which elements got made and by which stars dying, what planets
condensed from the leftovers, what the weather was like on them, and whether
anything ever started copying itself. Then it tells you what happened.

No graphics. No windows. Lines of text, in order, top to bottom. That is
deliberate and permanent — see "How this reads out loud" below.

## Running it

From this folder:

```bash
cargo run --release -- watch --seed hearth
```

The first run compiles it, which takes a minute. After that the program lives
at `target/release/lifesim.exe` and you can run it directly:

```bash
./target/release/lifesim.exe watch --seed hearth
```

Four commands:

| Command | What it does |
|---|---|
| `lifesim watch` | Runs a universe slowly enough to read as it goes. |
| `lifesim run` | The same, at full speed — about three seconds for 13 billion years. |
| `lifesim explore` | Walk around inside a running world. See below. |
| `lifesim guide` | Explains every term it uses, in plain language. |
| `lifesim help` | The options list. |

Useful options:

- `--seed hearth` — pick the universe. A word works as well as a number.
- `--detail brief` / `normal` / `deep` — how much it shows you.
- `--pace 700` — milliseconds of rest between paragraphs.
- `--log run.txt` — write everything to a file as well as the screen.
- `--persist` — keep simulating life until the star actually dies, instead of
  stopping after six billion years.
- `--narrator openrouter|ollama|builtin` — who does the telling. See below.
- `--model NAME` — which model to use for that.
- `--ollama-host URL` — where Ollama is, if not this machine.
- `--toast` — raise a Windows notification on each real event, so you can put
  the window in the background and be told when something happens.

The same seed always rebuilds exactly the same universe, on any machine,
forever. If you get one you like, write the number down. The last line of every
run tells you the number.

## Exploring

`lifesim run` reads you a summary. `lifesim explore` lets you stop and look.

```bash
lifesim explore --seed hearth
```

The run pauses every time something happens. At the prompt:

| | |
|---|---|
| *(enter)* | carry on to the next thing that happens |
| `go 50` | let fifty million years pass, whatever happens |
| `look NAME` | everything known about one lineage |
| `back NAME` | walk its ancestry, showing only where something changed |
| `kin NAME` | what split off from it |
| `life` | what is alive now, biggest first |
| `ocean` / `land` | what lives where |
| `world` | the planet right now: air, temperature, water, day length |
| `sky` | the star, its colour, its lifetime, this world's orbit |
| `run` | stop asking and go to the end |

The point is that all of this was always being computed and then thrown away.
Every lineage has a genome, a parent, sixteen traits, and a date it appeared.
`back` is the one worth trying first — it walks a creature up its own ancestry
and prints only the rungs where it became a different animal, so you can watch
a sessile single cell that was poisoned by oxygen turn into a dog-sized hunter
that breathes it.

Explore mode is terse by default and uses the built-in prose rather than a
language model, because stopping constantly to wait on a model would be slow
and would spend tokens narrating passages nobody asked to read. Pass
`--narrator openrouter` if you want it anyway.

## How this reads out loud

Everything about the output is built for a screen reader.

There are no progress bars, no spinners, no cursor tricks, no box-drawing
characters, no colour carrying meaning, and nothing that redraws in place.
Every line is written once and stays written. Timestamps come first on a line so
you can skim by them. Numbers are labelled as `label: value`. Long lines are
hard-wrapped so line navigation works.

If any of that ever stops being true, it is a bug, not a style choice.

## Who tells it

The simulation computes facts. Turning those facts into sentences is a separate
job, and it can be done two ways.

**The built-in prose** is written into the program. It is offline, instant,
deterministic, and has a fixed vocabulary — which means that over a run of
thirteen billion years it starts to sound like a form letter, because it is one.

**A language model** can do it instead. It is handed the computed facts for each
passage and asked to write them, along with a list of words and images it has
already spent this run so it stops reaching for the same ones. This is why the
same seed, run twice, reads differently while every number stays identical.

```bash
lifesim run --seed hearth --narrator openrouter
lifesim run --seed hearth --narrator ollama --model mistral:latest
lifesim run --seed hearth --narrator builtin
```

Ollama does not have to be on this machine. If there is a better one on the
network — more memory, faster models, reachable over a private tunnel — point
at it:

```bash
lifesim run --seed hearth --narrator ollama --ollama-host http://my-mac:11434 --model MODEL
```

`OLLAMA_HOST` is used when `--ollama-host` is not given. The connection check
allows a couple of seconds, since a machine across a tunnel answers more slowly
than one on the same desk. For this job, pick a model that can turn reasoning
off and is already resident — a model that reloads from disk on every request
will make a run take hours.

With no `--narrator` flag it picks the best thing available: OpenRouter if
`OPENROUTER_API_KEY` is set, else a local Ollama if one is answering, else the
built-in prose. It never fails hard — if a request errors, it says so once and
falls back.

**It uses a free model by default.** `--narrator openrouter` runs on
`minimax/minimax-m3:free`, which costs nothing. Nobody should discover they
have been billed for watching a universe because they forgot a flag. Pass
`--model anthropic/claude-sonnet-4.5` (or any other id) to use a paid one,
which is better again and costs a few cents a run.

The free default was picked by running real narration prompts through every
free model OpenRouter offered, at the batch size the program actually sends:

| Model | Result |
|---|---|
| `minimax/minimax-m3:free` | 6-23s, right format, every number preserved across repeated trials, visibly different wording each time. **Chosen.** |
| `z-ai/glm-5.2:free` | Capable, but rate-limited on every attempt. Kept as a fallback. |
| `google/gemma-4-31b-it:free` | Same. Second fallback. |
| `minimax/minimax-m2.7:free` | Writes the best prose of any of them, but its endpoint makes reasoning mandatory. On a full batch it spent twenty-five thousand tokens thinking and returned an empty message. Unusable. |
| `nvidia/nemotron-3-super-120b-a12b:free` | Returned the facts as a list instead of prose. |
| `nvidia/nemotron-3.5-lightning:free`, `nemotron-3-ultra-550b-a55b:free` | Ignored the output format. |
| `thinkingmachines/inkling:free` | Restricted to agentic harnesses. |
| `dots-studio/dots-3-note-preview:free`, `openrouter/free` | Returned nothing. |

A two-passage test picked the wrong winner. m2.7 looked best on a small prompt
and only failed at the size the program really uses, because a short prompt
left it enough budget to think *and* answer. Anything testing a model for this
kind of job has to test it at full size.

Several of these are reasoning models, so requests go out with reasoning
disabled; an endpoint that refuses is retried without the flag. Free endpoints
also rate-limit without warning, so if the current model stops answering
usefully the program says why and moves to the next one rather than silently
giving up on narration for the rest of the run.

**It is much slower.** The built-in prose finishes a universe in about three
seconds. A narrated run makes one network call per batch of passages and takes
several minutes, most of it spent on the chapter that lists every planet.
`--detail brief` cuts the number of passages and so cuts the time. If you want
to skim a lot of seeds looking for an interesting one, run them with
`--narrator builtin` and then re-run the good seed narrated — the universe will
be identical.

Three things worth knowing:

- **The model is never asked what happened.** It is given facts and told to
  preserve every number and name exactly. If it contradicts the simulation, the
  simulation is right. `--detail deep` prints the raw computed numbers
  underneath every passage, so you can check it.
- **Quality tracks the model.** Claude Sonnet through OpenRouter writes like a
  good science journalist. A 3-billion-parameter local model will drift — it
  will get a number wrong or add a flourish that is not in the facts. Local is
  free and private; remote is accurate and costs per run.
- **OpenRouter is a remote service.** Using it sends the passage facts — invented
  cosmology, nothing personal — to a third party, and bills your account. The
  API key is read from the environment and passed to curl in a config file, so
  it never appears in a command line or a process listing, and it is never
  written to the log.

## Notifications

`--toast` raises a Windows notification for each real event, so a long run can
be left in the background.

Only genuine events raise one — the milestones, the mass extinctions, the
ending. Status lines, planet listings and descriptive passages never do, and
there is a hard limit of one notification every four seconds with anything
faster dropped rather than queued.

That restraint is the point. A screen reader announces toasts out loud, which
is exactly what makes them useful here and exactly what makes flooding them
unbearable. If it ever gets chatty, that is a bug.

It uses PowerShell's registered notification identity rather than installing
one of its own, because a simulation should not be writing to anybody's Start
Menu. If notifications do not appear, check that Focus Assist or Do Not Disturb
is off; the program cannot tell whether Windows actually displayed one.

## What is actually being simulated

The point of this program is that everything downstream depends on everything
upstream, causally, with no step written in advance. The chain runs:

**How fast the universe expands and how lumpy it starts** decides whether gas
ever collapses at all. Some universes never form a single galaxy, and those runs
end early and say so.

**How much helium got fused in the first twenty minutes** decides what stars are
made of, and a universe that burnt too much of its hydrogen gets only stars that
live fast and die young.

**Which stars died** decides what elements exist. There was no carbon, no
oxygen, no iron and no phosphorus until specific stars made them and gave them
back. The program tracks ten elements accumulating in the gas over twelve
billion years, and iron arrives late relative to oxygen because it comes mostly
from white dwarfs detonating long after their birth.

**How much iron and silicon the galaxy has made** decides how much solid dust a
planet-forming disk gets, which decides how large planetary embryos can grow,
which decides whether any of them reach the ten Earth masses needed to start
pulling in gas and become a giant planet.

**Whether there are giant planets, and where** decides whether the inner rocky
worlds ever get any water, because they formed dry and their oceans have to be
thrown in from the cold outer system by something massive.

**Whether a rocky world has plate tectonics** decides whether it has a
thermostat, chemical gradients to eat, and a magnetic field.

**And then life**, if it starts, spends four billion years rewriting all of it.

## The part about life

There are no scripted outcomes. Nothing in the program says "after two billion
years, invent multicellularity."

What exists instead is genomes — lists of genes that mutate, get duplicated,
get deleted, get swapped sideways between lineages, and occasionally get merged
wholesale when one cell swallows another and fails to digest it. Genes map to
traits. Traits are *gated*: a nervous system cannot be expressed without a body
to put it in, a body cannot be expressed without the oxygen budget to run it,
and oxygen does not exist until something has been making it as a waste product
for a billion years.

Fitness is energy income minus energy cost, matched against an environment. The
environment is being continuously rewritten by the life in it — oxygen
accumulating, methane being destroyed, carbon dioxide being drawn down, ozone
forming and shutting off the ultraviolet that made the land lethal.

So the order events happen in comes out of the energy budget, not out of a plan.
Which is why runs where oxygen never accumulates stay microbial for eight
billion years and then the star kills them — and most runs do.

Roughly, out of twenty-four universes: about a third get as far as animals with
nervous systems, about a third stall at microbes breathing an oxygen
atmosphere, and one or two produce something that works out where it came from.
The rest land somewhere between. That distribution is not tuned to a target; it
is what the numbers do.

## What is approximate, and how much

Being clear about this, because "physics-accurate" can mean a lot of things.

**Genuinely computed from real physics:** the expansion history (a numerical
integration of the Friedmann equation), temperature and redshift relations,
matter-radiation equality, the mass-luminosity relation for main-sequence stars,
stellar lifetimes, effective temperatures via Stefan-Boltzmann, the Kroupa
initial mass function, the planetary isolation mass, the snow line, equilibrium
temperatures, escape velocities, tidal locking timescales, orbital periods.

**Fitted to real results rather than derived:** the nucleosynthesis yields, the
supernova element yields, the density-fluctuation spectrum, the mass-radius
relations for planets, the greenhouse and weathering laws.

**Honestly a model, with numbers chosen for plausible behaviour:** everything
about life. The odds of life starting are a guess, because nobody knows them.
The trait system is a caricature of genetics — sixteen channels, not twenty
thousand genes. The ecology is replicator dynamics with niche competition, not a
spatial ecosystem.

**Deliberately not modelled:** spatial structure of the galaxy, orbital
resonance and long-term dynamical stability, radiative transfer in atmospheres,
actual biochemistry, anything after a civilisation appears.

The narration never invents an event. Every line describes something the
simulation computed. When it says the air is twelve percent oxygen, that number
came out of a photosynthesis budget minus a respiration budget minus a
weathering sink.

## Layout

| File | What lives there |
|---|---|
| `src/rng.rs` | Deterministic randomness. Same seed, same universe, forever. |
| `src/units.rs` | Physical constants, and turning numbers back into human words. |
| `src/narrate.rs` | The voice. All output goes through here. |
| `src/cosmos.rs` | Planck instant to the first collapsing cloud. |
| `src/stars.rs` | First light, stellar deaths, and the making of the elements. |
| `src/planets.rs` | Disks, planet formation, migration, climate. |
| `src/life.rs` | Genomes, traits, ecology, and the feedback into the planet. |
| `src/llm.rs` | Optional live narration through Ollama or OpenRouter. |
| `src/toast.rs` | Windows notifications, rate-limited on purpose. |
| `src/main.rs` | The command line, and the run that stitches the acts together. |

Every file starts with a comment explaining what it is doing and why, including
the parts that are approximations and the reasons for them.

## License

CC0 1.0 Universal. The author has waived all copyright and related rights in
this work worldwide, to the extent allowed by law. See `LICENSE` for the full
dedication.

In plain terms: it is in the public domain. Take it, change it, build on it,
ship it, sell it, relicense it, claim it, or ignore it. No permission needed,
no attribution required, no conditions attached.
