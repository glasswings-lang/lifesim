# Bodies that the simulation actually grows

*A design note, not yet built.*

## What is wrong now

Right now a creature's body is a **rendering**, not a thing.

The genome makes sixteen trait numbers. Those numbers go into a function that
picks sentences: high motility in water prints "one flick of the whole body,
then a coast"; a dim red star prints "no eyes worth the name, feelers instead".

Every one of those rules was written by hand. The creature does not *have*
feelers. Nothing in the world knows what a feeler is. It has `sensing = 0.71`
and a lookup table decided how to say that out loud.

Which means: every creature in every world is drawn from the same few dozen
sentences. Two animals with the same trait numbers are identical, in every
universe, forever. There is no possibility of a body plan nobody anticipated,
because the set of describable bodies is a list somebody typed.

That is the opposite of how the rest of this program works. The oxygen
catastrophe is not scripted; it happens because photosynthesis has a waste
product and the sea eventually saturates. Bodies should be like the oxygen.

## The inversion

Today:

    genome -> traits -> fitness
                     -> description (a costume, bolted on the side)

Proposed:

    genome -> a developmental program -> a body -> traits -> fitness
                                             `-> description (read off the body)

The body becomes the load-bearing object. Traits stop being primary data and
become *measurements of a body*. Motility is not a number the genome states; it
is what you get when you count propulsive surfaces and divide by mass. Sensing
is how many sense organs there are and where they sit. Manipulation is whether
anything on the animal can close.

Then bodies evolve, because bodies are what selection can reach.

## The encoding

Keep it small. This is not a physics engine and should not become one.

A genome gains a **plan** alongside its existing genes:

- **symmetry** — radial(n) | bilateral | none
- **segments** — a count, and a size gradient (each successive segment scaled
  by some factor, so animals taper or swell)
- **appendage rules**, keyed by position band (front / middle / rear) rather
  than by individual segment, so that a mutation can change a whole region at
  once: `none | paddle | leg | grasper | frond | spine | filament`
- **organ rules** — where sense organs sit, how many, and what kind
  (light-sensitive patch, chemical pit, movement filament); where the feeding
  aperture is, and how wide it opens
- **surface** — one gene, read together with habitat and desiccation tolerance

Development is then a short deterministic function: run the rules, get a
concrete `Body { segments: Vec<Segment> }` where each segment carries its own
size, appendages and organs. No recursion depth worth worrying about, no
physics, and it terminates.

## Where the world gets a vote

Development should read the planet, so the same genome grows differently in
different places:

- **Gravity** caps how much mass a given limb cross-section can hold up. Heavy
  world: fewer, thicker, more numerous supports, or stay in the water. Light
  world: long spindly things are viable.
- **Medium** decides what a propulsive surface is worth. Paddles are excellent
  in water and useless on land; legs are the reverse.
- **Available light** decides whether a light-sensitive organ pays for itself
  at all. Under a dim red star it does not, so eyes stay vestigial and
  movement-sensing filaments win — which is the thing I currently hardcode, and
  which should instead simply *fall out*.
- **Oxygen** limits total mass that can be supplied, exactly as it already
  limits everything else.

## What falls out of it

- `MOTIL` = propulsive surface area over mass, weighted by medium
- `SENSE` = sense organ count, weighted by placement
- `MANIP` = graspers present, gated on nervous complexity
- `SIZE` = total mass, straight from the segments
- `MULTI` = segment count and how differentiated the segments are from one
  another
- `PHOTO` = frond surface area facing the light

Note that several of these stop needing their own genes. The genome gets
*smaller* and the phenotype gets richer, which is the correct direction.

## Why it is worth doing

**Segment duplication.** The single most important mutation available becomes
"copy a body region". That is not a gimmick — it is roughly how arthropods
generated their entire range of forms, and the genes that do it are real. A
copied region is free to specialise afterwards, exactly like a duplicated gene
is, and the program already has that idea for genes. It should have it for
bodies.

**Real convergence.** Two unrelated lineages arriving independently at the same
body plan would mean something, because neither was chosen from a list.

**Descriptions become readings.** "Seven segments, tapering; paired paddles on
the last four; a ring of chemical pits around the mouth" is not a sentence
anyone wrote. It is a description of a data structure. It is also, unlike the
current output, *different every time*.

**It answers the actual complaint.** "I don't know what they even look like"
is not solved by better sentences about trait numbers. It is solved by there
being something there to look at.

## Risks

- **Scope creep into a physics engine.** Resist. Development is a short pure
  function from plan to body; no simulation of forces, no collision, no
  biomechanics solver.
- **Degenerate winners.** If one plan is strictly best, everything converges to
  it and the variety is lost. The existing niche-crowding term should handle
  this, but it needs checking against bodies rather than against traits.
- **Unreadable output.** A body with forty appendage types is not more vivid
  than one with three, it is just noisy. Cap the vocabulary deliberately.
- **Losing the honest failure modes.** Most worlds must still stay microbial.
  A body plan system must not accidentally make complexity cheap.

## Where to build it

This is a rewrite of the middle of `life.rs`, and it is the right moment to do
it in Python instead — the parts that need frequent tinkering here are exactly
the parts that are painful to iterate on in Rust, and the interface already
talks to the simulation over a line protocol, so what is underneath can be
swapped without touching the window.

Do it fresh, with this file open. Not at the end of a long session.
