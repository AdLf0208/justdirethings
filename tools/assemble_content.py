#!/usr/bin/env python3
"""Assemble content/ for Just Dire Things slice 1 (the Simple Coal Generator).

`content/` **is** the mod root (`horizon-mod.toml` + `assets/` + `data/`); this
script rebuilds it from the pack jar deterministically:

    python3 tools/assemble_content.py

Carried byte-for-byte from `justdirethings-1.5.7.jar`: the blockstate, the block
model, the item models of the generator and the four coals, the three block
textures and the four item textures.

Written (the 26.3 forms the carry pipeline would produce, by hand because the
jar is 1.21.1): the `items/` item definitions, the block/item declarations from
vanilla templates, the loot table, the recipe and its advancement, the creative
tab, the pickaxe tag and the `en_us` subset.

Every choice is recorded in CONTENT.md.
"""
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/justdirethings-1.5.7.jar"
CONTENT = ROOT / "content"
NS = "justdirethings"

CARRIED = [
    "assets/justdirethings/blockstates/generatort1.json",
    "assets/justdirethings/models/block/generatort1.json",
    "assets/justdirethings/models/item/generatort1.json",
    "assets/justdirethings/models/item/coal_t1.json",
    "assets/justdirethings/models/item/coal_t2.json",
    "assets/justdirethings/models/item/coal_t3.json",
    "assets/justdirethings/models/item/coal_t4.json",
    "assets/justdirethings/textures/block/generatort1_side.png",
    "assets/justdirethings/textures/block/generatort1_top.png",
    "assets/justdirethings/textures/block/generatort1_bottom.png",
    "assets/justdirethings/textures/item/coal_t1.png",
    "assets/justdirethings/textures/item/coal_t2.png",
    "assets/justdirethings/textures/item/coal_t3.png",
    "assets/justdirethings/textures/item/coal_t4.png",
]

# 26.3 item definitions: a model, nothing else.


# NeoForge's data/neoforge/data_maps/item/furnace_fuels.json, now the item's own
# `minecraft:cooking_fuel` component (burn_time ticks, speed_multiplier).
COALS = {
    "coal_t1": (4800, 2.0),
    "coal_t2": (14400, 4.0),
    "coal_t3": (43200, 8.0),
    "coal_t4": (129600, 16.0),
}

LANG = {
    "itemGroup.justdirethings": "Just Dire Things",
    "block.justdirethings.generatort1": "Simple Coal Generator",
    "item.justdirethings.coal_t1": "Primal Coal",
    "item.justdirethings.coal_t2": "Blaze Ember",
    "item.justdirethings.coal_t3": "Voidflame Coal",
    "item.justdirethings.coal_t4": "Eclipse Ember",
}

# The jar's loot table already has 26.3's singular condition.
LOOT = {
    "type": "minecraft:block",
    "pools": [
        {
            "bonus_rolls": 0.0,
            "conditions": [{"condition": "minecraft:survives_explosion"}],
            "entries": [{"type": "minecraft:item", "name": f"{NS}:generatort1"}],
            "rolls": 1.0,
        }
    ],
    "random_sequence": f"{NS}:blocks/generatort1",
}

# The jar's shaped recipe, ingredients in 26.3's form. `justdirethings:ferricore_ingot`
# is not carried by this slice, so the recipe ships and does not match yet.
RECIPE = {
    "type": "minecraft:crafting_shaped",
    "category": "misc",
    "group": "justdirethings",
    "key": {
        "c": "minecraft:coal",
        "f": "minecraft:blast_furnace",
        "i": f"{NS}:ferricore_ingot",
        "r": "minecraft:redstone",
    },
    "pattern": ["iri", "cfc", "iri"],
    "result": {"count": 1, "id": f"{NS}:generatort1"},
}


def write(rel, value):
    path = CONTENT / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, (dict, list)):
        path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n")
    else:
        path.write_text(value)


def main():
    jar = zipfile.ZipFile(JAR)
    for name in CARRIED:
        data = jar.read(name)
        target = CONTENT / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)

    # 26.3 item definitions for the block item and the four fuels.
    for name in ("generatort1", *COALS):
        write(
            f"assets/{NS}/items/{name}.json",
            {"model": {"type": "minecraft:model", "model": f"{NS}:item/{name}"}},
        )
    write(f"assets/{NS}/lang/en_us.json", LANG)

    # The block: upstream's full-cube shape, strength 2.0, metal sounds, and a
    # block entity; vanilla's iron block has none of its own state properties and
    # the jar's blockstate has the single "" variant, so they match.
    write(
        f"data/{NS}/horizon/block/generatort1.json",
        {
            "template": "minecraft:iron_block",
            "properties": {
                "hardness": 2.0,
                "explosion_resistance": 2.0,
                "sounds": "minecraft:iron_block",
                "requires_tool": False,
            },
            "item": True,
        },
    )
    for name, (burn_time, speed) in COALS.items():
        write(
            f"data/{NS}/horizon/item/{name}.json",
            {
                "template": "minecraft:coal",
                "components": {
                    "minecraft:cooking_fuel": {
                        "burn_time": burn_time,
                        "speed_multiplier": speed,
                    }
                },
            },
        )

    write(f"data/{NS}/loot_table/blocks/generatort1.json", LOOT)
    write(f"data/{NS}/recipe/generatort1.json", RECIPE)
    write(
        f"data/{NS}/horizon/creative_tab/machines.json",
        {
            "title": "Just Dire Things",
            "icon": f"{NS}:generatort1",
            "items": [f"{NS}:generatort1", *(f"{NS}:{name}" for name in COALS)],
        },
    )
    write(
        "data/minecraft/tags/block/mineable/pickaxe.json",
        {"values": [f"{NS}:generatort1"]},
    )
    files = sorted(p.relative_to(CONTENT) for p in CONTENT.rglob("*") if p.is_file())
    print(f"{len(files)} files:")
    for name in files:
        print(f"  {name}")


if __name__ == "__main__":
    main()
