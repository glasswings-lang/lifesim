"""
to_tff.py -- turn a creature that evolved in lifesim into a Time for Family species.

lifesim grows animals nobody designed: they have a name, a body, a way of making
a living, a habitat, ancestors, and a world that shaped all of it. Time for
Family takes creatures in and gives them somewhere to live. This carries one
across.

    python to_tff.py                      pick the most interesting creature
    python to_tff.py Breistinn            pick that one by name
    python to_tff.py --list               show what is available
    python to_tff.py --world other.json   use a different world

It writes two things into the tff repo:

    assets/types/species/<id>.json        the species definition
    assets/text/species/<id>/*.txt        the flavour text pools

Nothing is overwritten without --force.

---------------------------------------------------------------------------
One file is deliberately left empty: disabilities.txt
---------------------------------------------------------------------------
Time for Family says disability is "represented with respect, woven into the
creatures naturally rather than as a problem to fix." That is a thing a person
decides, about a creature they know, in their own words. Generating it
automatically out of a physics simulation would be exactly the tone-deaf move
the game was built to avoid, so the file is created with its header and left
for you to write. Everything else here is a starting point you can edit; that
one is not mine to start.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_WORLD = os.path.join(HERE, "world.json")
DEFAULT_TFF = os.path.abspath(os.path.join(HERE, "..", "tff"))

HOUR = 3600


# --------------------------------------------------------------------------
# reading the world
# --------------------------------------------------------------------------

def load(path: str) -> dict:
    if not os.path.isfile(path):
        sys.exit(f"No world at {path}.\n"
                 f"Make one first:\n"
                 f"  target\\release\\lifesim.exe run --seed hearth --dump world.json")
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def interesting(c: dict) -> float:
    """How much of a creature something is, as opposed to a smear of cells."""
    t = c["traits"]
    return (t["body size"] * 3 + t["nervous complexity"] * 4
            + t["multicellularity"] * 2 + t["motility"] + t["sensing"]
            + c["share"])


# --------------------------------------------------------------------------
# names, in the same voice as the world they came from
# --------------------------------------------------------------------------

ONSETS = ["th", "k", "s", "m", "r", "v", "l", "n", "d", "p", "br", "kr", "st",
          "sh", "tr", "gl", "z", "f", "h", "y", "w", "ch"]
NUCLEI = ["a", "e", "i", "o", "u", "ae", "ei", "ou", "ia", "au", "yu", "eo"]
CODAS = ["n", "s", "r", "l", "th", "m", "k", "sh", "ll", "nn", "rr", "ph", "st"]


def coin(rng: random.Random, syllables: int | None = None) -> str:
    n = syllables or rng.randint(2, 3)
    out = ""
    for i in range(n):
        out += rng.choice(ONSETS) + rng.choice(NUCLEI)
        if i == n - 1 and rng.random() < 0.6:
            out += rng.choice(CODAS)
    return out.capitalize()


def name_pool(rng: random.Random, count: int) -> list[str]:
    seen: list[str] = []
    while len(seen) < count:
        n = coin(rng)
        if n not in seen:
            seen.append(n)
    return seen


# --------------------------------------------------------------------------
# turning traits into a creature somebody could keep
# --------------------------------------------------------------------------

def room_for(c: dict, world: dict) -> list[str]:
    if not c["lives_on_land"]:
        return ["aquatic"]
    if c["traits"]["motility"] > 0.6:
        return ["outdoor", "glade"]
    return ["glade"]


def lifespan_seconds(c: dict) -> int:
    """Bigger, slower-breeding animals live longer. Roughly a day for a small
    one, several days for a large one, in the game's own time."""
    size = c["traits"]["body size"]
    return int((18 + size * 90) * HOUR)


def describe(c: dict, world: dict, star: dict) -> str:
    t = c["traits"]
    bits = []
    if t["body size"] > 0.7:
        bits.append("Large and unhurried")
    elif t["body size"] > 0.45:
        bits.append("About the size of a cat, and solid with it")
    elif t["multicellularity"] > 0.4:
        bits.append("Small enough to sit in two hands")
    else:
        bits.append("Barely more than a soft clump, and quite content that way")

    if t["motility"] > 0.65:
        bits.append("moves in fast bursts and then stops dead")
    elif t["motility"] > 0.35:
        bits.append("moves slowly and thinks about it first")
    else:
        bits.append("stays where it is put and does very well there")

    if t["phototrophy"] > 0.5:
        bits.append("and feeds on light, so a bright corner is most of what it wants")
    elif t["nervous complexity"] > 0.45:
        bits.append("and watches things for far longer than you expect")
    elif c["hunts"]:
        bits.append("and would rather find its food than be handed it")
    else:
        bits.append("and is not fussy about much")

    where = "water" if not c["lives_on_land"] else "open ground"
    oceans = world["oceans"]
    water = ("a world almost entirely ocean" if oceans > 50
             else f"a world of about {oceans:.1f} Earth oceans" if oceans > 0.3
             else "a mostly dry world")
    origin = (f"It evolved on {water}, under {star['colour']} light, and it is "
              f"built for {where}.")
    # The simulation now writes a physical description of its own, worked out
    # from the same traits: size against something you have held, how it moves,
    # what its surface would feel like, whether it has eyes at all. That is far
    # better than anything assembled out here, so prefer it when it is present.
    body = c.get("body")
    if body:
        return f"{body} {origin}"
    # The fragments are clauses, not sentences. Joining them with full stops
    # produced "Large and unhurried. moves in fast bursts."
    return f"{bits[0]}, {bits[1]}, {bits[2]}. {origin}"


def needs_lines(c: dict, world: dict) -> list[str]:
    t = c["traits"]
    out = []
    if not c["lives_on_land"]:
        out.append("Water deep enough to sink into and not be looked at for a while.")
    else:
        out.append("Ground it can get its feet properly under.")
    if t["cold tolerance"] > 0.6:
        out.append("Somewhere cool. It gets sluggish and cross when it is warm.")
    if t["heat tolerance"] > 0.6:
        out.append("Warmth. Real warmth, the kind that soaks in.")
    if t["phototrophy"] > 0.4:
        out.append("Light on it for part of the day. It is half plant about this.")
    if t["motility"] > 0.55:
        out.append("Room to move fast in a straight line, even briefly.")
    if t["sensing"] > 0.5:
        out.append("Things to notice. It goes flat and dull without them.")
    if t["nervous complexity"] > 0.45:
        out.append("Something to work out. It will find a problem if you do not give it one.")
    if t["sociality"] > 0.4:
        out.append("Company of its own kind, or failing that, yours.")
    if t["symbiosis"] > 0.5:
        out.append("It does better sharing a space than having one to itself.")
    out.append("Being left alone sometimes, without that meaning anything is wrong.")
    return out


def pet_lines(c: dict) -> list[str]:
    t = c["traits"]
    out = []
    if t["nervous complexity"] > 0.45:
        out += ["It looks up, works out that this is you, and settles.",
                "It leans in and stays leaning longer than is dignified."]
    if t["motility"] > 0.55:
        out += ["It goes still all at once, which for this one is affection.",
                "It bolts two steps, thinks better of it, and comes back."]
    if t["sensing"] > 0.5:
        out.append("It turns whatever it senses with toward you and holds it there.")
    if t["phototrophy"] > 0.5:
        out.append("It spreads itself a little wider, the way it does in good light.")
    out += ["It accepts this. That is the whole response and it is enough.",
            "Nothing visible happens, but it does not move away."]
    return out


def colour_lines(rng: random.Random, star: dict) -> list[str]:
    """Colours that make sense under this world's own sun. Not decoration:
    what an animal looks like depends on the light it evolved under."""
    warm = "red" in star["colour"] or "orange" in star["colour"]
    if warm:
        base = ["deep rust", "dark plum", "near-black with a red sheen",
                "burnt brown", "dull copper", "smoke and ember", "old iron"]
    elif "blue" in star["colour"] or "white" in star["colour"]:
        base = ["chalk white", "pale silver", "washed grey", "cold blue-white",
                "bleached bone", "faint violet", "clear and almost colourless"]
    else:
        base = ["moss green", "wet sand", "dark olive", "pale gold", "grey-brown",
                "green so dark it reads black", "dappled tan"]
    rng.shuffle(base)
    return base


# --------------------------------------------------------------------------
# writing it out
# --------------------------------------------------------------------------

def write_pool(path: str, header: list[str], lines: list[str]) -> None:
    with open(path, "w", encoding="utf-8") as f:
        for h in header:
            f.write(f"# {h}\n")
        f.write("\n")
        for l in lines:
            f.write(l + "\n")


def export(c: dict, data: dict, tff: str, force: bool) -> None:
    world, star = data["world"], data["star"]
    ident = "".join(ch for ch in c["name"].lower() if ch.isalpha()) or "creature"
    rng = random.Random(f"{data['seed']}:{c['name']}")

    types_dir = os.path.join(tff, "assets", "types", "species")
    text_dir = os.path.join(tff, "assets", "text", "species", ident)
    if not os.path.isdir(types_dir):
        sys.exit(f"That does not look like the tff repo: {tff}\n"
                 f"Point at it with --tff PATH")
    spec_path = os.path.join(types_dir, f"{ident}.json")
    if os.path.exists(spec_path) and not force:
        sys.exit(f"{ident} already exists in Time for Family. Use --force to replace it.")
    os.makedirs(text_dir, exist_ok=True)

    t = c["traits"]
    life = lifespan_seconds(c)
    fecund = t["fecundity"]
    spec = {
        "name": c["name"],
        "name_plural": c["name"] + ("es" if c["name"][-1] in "sxz" else "s"),
        "id": ident,
        "sex_label_male": "male",
        "sex_label_female": "female",
        "care_action_label": "Sit with",
        "litter_label": "brood",
        "litter_label_plural": "broods",
        "starter_age_min_seconds": int(life * 0.20),
        "starter_age_max_seconds": int(life * 0.40),
        "starter_pairs": 1,
        "breeding_age_seconds": int(life * 0.25),
        "elder_age_seconds": int(life * 0.75),
        "gestation_seconds": int(1200 + 3600 * t["body size"]),
        "mother_dependency_seconds": int(2400 + 5400 * t["body size"]),
        "twin_chance": round(min(0.45, 0.05 + fecund * 0.4), 2),
        "sex_short_female": "F",
        "sex_short_male": "M",
        "compatible_room_types": room_for(c, world),
        "text_directory": ident,
        "name_generation": "markov",
        "min_babies": 1 if t["body size"] > 0.5 else 2,
        "max_babies": max(2, int(2 + fecund * 5)),
        "description": describe(c, world, star),
    }
    with open(spec_path, "w", encoding="utf-8") as f:
        json.dump(spec, f, indent=2, ensure_ascii=False)
        f.write("\n")

    origin = (f"{c['name']}, evolved in lifesim world {data['seed']}. "
              f"{c['description']}")

    write_pool(os.path.join(text_dir, "descriptions.txt"),
               [f"{c['name']} descriptions. One per line.", origin,
                "Edit freely -- these are a starting point, not a fixed set."],
               [describe(c, world, star)] + ([c["body"]] if c.get("body") else []) + [
                   "It has the particular stillness of something that was never "
                   "in a hurry to begin with.",
                   "Up close it is stranger than it looks from across a room.",
                   "It arranges itself into whatever shape the space suggests.",
               ])
    write_pool(os.path.join(text_dir, "needs.txt"),
               [f"What {spec['name_plural'].lower()} need looking after.",
                "Drawn from what this animal actually evolved for."],
               needs_lines(c, world))
    write_pool(os.path.join(text_dir, "pet_responses.txt"),
               [f"How a {c['name']} responds to being sat with."],
               pet_lines(c))
    write_pool(os.path.join(text_dir, "colors.txt"),
               [f"Colours a {c['name']} comes in.",
                f"Chosen for the light it evolved under: {star['colour']}."],
               colour_lines(rng, star))
    write_pool(os.path.join(text_dir, "names_female.txt"),
               [f"Female {spec['name_plural'].lower()} names.",
                "Generated in the same voice as the world they came from."],
               name_pool(rng, 22))
    write_pool(os.path.join(text_dir, "names_male.txt"),
               [f"Male {spec['name_plural'].lower()} names."],
               name_pool(rng, 22))
    write_pool(os.path.join(text_dir, "disabilities.txt"),
               [f"Ways a {c['name']} can be built differently.",
                "",
                "Deliberately left empty.",
                "",
                "Time for Family represents disability with respect, woven in",
                "naturally rather than as a problem to fix. That is a thing a",
                "person decides, about a creature they know, in their own words.",
                "Generating it out of a physics simulation would be the exact",
                "tone-deaf move this game was built to avoid.",
                "",
                "So this one is yours to write."],
               [])

    print(f"Wrote {c['name']} into Time for Family.")
    print(f"  {spec_path}")
    print(f"  {text_dir}\\  (7 files)")
    print(f"  lives in: {', '.join(spec['compatible_room_types'])}")
    print()
    print("disabilities.txt is empty on purpose. See the note inside it.")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("creature", nargs="?", help="which one, by name")
    ap.add_argument("--world", default=DEFAULT_WORLD)
    ap.add_argument("--tff", default=DEFAULT_TFF)
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--force", action="store_true")
    a = ap.parse_args()

    data = load(a.world)
    creatures = data["creatures"]

    if a.list:
        ranked = sorted(creatures, key=interesting, reverse=True)
        print(f"World {data['seed']} -- {len(creatures)} creatures. "
              f"The most creaturely first:\n")
        for c in ranked[:20]:
            print(f"  {c['name']:<16} {c['description'][:78]}")
        return

    if a.creature:
        wanted = a.creature.lower()
        match = next((c for c in creatures if c["name"].lower() == wanted), None)
        if match is None:
            match = next((c for c in creatures
                          if c["name"].lower().startswith(wanted)), None)
        if match is None:
            sys.exit(f"No creature called {a.creature}. Try --list.")
    else:
        match = max(creatures, key=interesting)

    export(match, data, a.tff, a.force)


if __name__ == "__main__":
    main()
