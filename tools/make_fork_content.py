"""Writes Hostile Neural Networks' assets/, data/ and src/models_data.rs for the fork build (hostilenetworks-fork/),
from the original jar (HostileNeuralNetworks-26.1.2-7.0.0.jar).

What it keeps as the jar has it: textures, block and item models, item definitions (the prediction's
`hostilenetworks:model_color` tint and the data model's `hostilenetworks:data_model` node, both answered by the mod's
code), lang files, recipes, loot tables and tags.

What it writes for the fork:
- the block and item declarations (`data/hostilenetworks/sunrise/...`): Hostile.Blocks' strength and light, no occlusion
  for the three machines, a horizontal `facing` (SimChamberBlock, LootFabBlock, DataCenterBlock) and the IO port's `mode`;
  Hostile.Items' stack sizes;
- the three machines' menu types (slots and data slots);
- `data/c/tags`: the `c:` tags the recipes use, which vanilla (and so the fork) doesn't define;
- `src/models_data.rs`: the data models and tiers as Rust tables (DataModelRegistry, ModelTierRegistry), because the
  fork's datapack registries take flat records only and HNN's data models are nested (display, drop lists, per-tier
  maps). Their files are left out of `data/` so a datapack can't seem to change them.

Recipes with a `neoforge:mod_loaded` condition for another mod are left out (their items don't exist without it).

Usage: python make_fork_content.py [jar]
"""

import json
import os
import re
import shutil
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
ID = "hostilenetworks"
JAR = os.path.join(ROOT, "..", "Ported", "HostileNeuralNetworks-26.1.2-7.0.0.jar")

# Hostile.Blocks: lightLevel 1, strength(4, 3000) / strength(5, 6000), noOcclusion, not a redstone conductor. The
# properties name no sound, so they keep Block.Properties.of()'s stone; none asks for the right tool.
MACHINES = {
    "sim_chamber": (4.0, 3000.0),
    "loot_fabricator": (4.0, 3000.0),
    "data_center": (5.0, 6000.0),
}

# Hostile.Items: stacksTo(1) for these, 64 for the rest.
ITEMS = {
    "blank_data_model": 1,
    "prediction_matrix": 64,
    "overworld_prediction": 64,
    "nether_prediction": 64,
    "end_prediction": 64,
    "twilight_prediction": 64,
    "deep_learner": 1,
    "data_model": 1,
    "prediction": 64,
    "fab_directive": 1,
}

C_TAGS = {
    "obsidians": ["minecraft:obsidian", "minecraft:crying_obsidian"],
    "glass_panes": ["minecraft:glass_pane", "minecraft:black_stained_glass_pane", "minecraft:white_stained_glass_pane"],
    "ingots/gold": ["minecraft:gold_ingot"],
    "ingots/iron": ["minecraft:iron_ingot"],
    "ingots/netherite": ["minecraft:netherite_ingot"],
    "dusts/redstone": ["minecraft:redstone"],
    "gems/diamond": ["minecraft:diamond"],
    "gems/lapis": ["minecraft:lapis_lazuli"],
}

# The menu types: their slots and data slots (see src/sim.rs, fab.rs, center.rs for what each data slot holds).
MENUS = {
    "sim_chamber": {"slots": 4, "data_slots": 7, "results": [2, 3]},
    "loot_fabricator": {"slots": 17, "data_slots": 4, "results": list(range(1, 17))},
    "data_center": {"slots": 45, "data_slots": 54, "results": list(range(29, 45))},
}

TIER_ORDER = ["faulty", "basic", "advanced", "superior", "self_aware"]

# ChatFormatting's named colours, for TextColor.parseColor.
NAMED = {
    "black": 0x000000, "dark_blue": 0x0000AA, "dark_green": 0x00AA00, "dark_aqua": 0x00AAAA, "dark_red": 0xAA0000,
    "dark_purple": 0xAA00AA, "gold": 0xFFAA00, "gray": 0xAAAAAA, "dark_gray": 0x555555, "blue": 0x5555FF,
    "green": 0x55FF55, "aqua": 0x55FFFF, "red": 0xFF5555, "light_purple": 0xFF55FF, "yellow": 0xFFFF55,
    "white": 0xFFFFFF,
}


def color(value):
    if value.startswith("#"):
        return int(value[1:], 16) & 0xFFFFFF
    return NAMED.get(value, 0xFFFFFF)


def machine(name):
    hardness, resistance = MACHINES[name]
    return {
        "template": "minecraft:iron_block",
        "properties": {
            "hardness": hardness,
            "explosion_resistance": resistance,
            "sounds": "minecraft:stone",
            "light": 1,
            "requires_tool": False,
            "occludes": False,
        },
        "state_properties": {"facing": {"type": "enum", "values": ["north", "south", "west", "east"]}},
        "default_state": {"facing": "north"},
    }


def io_port():
    return {
        "template": "minecraft:iron_block",
        "properties": {"hardness": 5.0, "explosion_resistance": 6000.0, "sounds": "minecraft:stone", "requires_tool": False},
        "state_properties": {"mode": {"type": "enum", "values": ["energy", "models", "inputs", "outputs"]}},
        "default_state": {"mode": "energy"},
    }


def rust_str(s):
    return json.dumps(s, ensure_ascii=False)


def opt_i(v):
    return "None" if v is None else f"Some({int(v)})"


def models_source(models, tiers):
    """src/models_data.rs: ModelTierRegistry and DataModelRegistry as constants."""
    out = [
        "// Written by tools/make_fork_content.py from the jar's data/hostilenetworks/hostilenetworks: don't edit.",
        "",
        "use crate::models::{ModelDef, TierDef};",
        "",
        "/// ModelTierRegistry, sorted by required data.",
        "pub(crate) const TIERS: &[TierDef] = &[",
    ]
    for name in sorted(tiers, key=lambda n: tiers[n]["required_data"]):
        t = tiers[name]
        gained = t.get("data_gained", t.get("data_per_kill", 0))
        out.append(
            f"    TierDef {{ name: {rust_str(name)}, required: {t['required_data']}, gained: {gained}, "
            f"color: 0x{color(t['color']):06X}, accuracy: {float(t['accuracy'])!r}, can_sim: {str(t.get('can_sim', True)).lower()} }},"
        )
    out.append("];")
    out.append("")
    out.append("/// DataModelRegistry: every data model file of the jar, by id.")
    out.append("pub(crate) const MODELS: &[ModelDef] = &[")
    for key in sorted(models):
        m = models[key]
        is_block = "block" in m
        source = m["block"] if is_block else m["entity"]
        if isinstance(source, dict):
            source = source["block"]
        variants = [v["block"] if isinstance(v, dict) else v for v in m.get("variants", [])]
        display = m.get("display", {})
        per_tier = lambda d: "[" + ", ".join(opt_i((d or {}).get(t)) for t in TIER_ORDER) + "]"
        gained = m.get("data_gained", m.get("data_per_kill"))
        drops = []
        for d in m["fabricator_drops"]:
            drops.append(f"({rust_str(d['id'])}, {int(d.get('count', 1))}, {str(bool(d.get('optional', False))).lower()})")
        conditions = m.get("neoforge:conditions", [])
        needs = [c["modid"] for c in conditions if c.get("type") == "neoforge:mod_loaded"]
        block_cond = any(c.get("type") == f"{ID}:block_data_models_enabled" for c in conditions)
        not_silk = any(c.get("name") == f"{ID}:not_silk_touch" for c in m.get("upgrade_conditions", []))
        name = m.get("name")
        name_rs = "None" if name is None else f"Some({rust_str(json.dumps(name))})"
        base = m["base_drop"]
        out.append(
            "    ModelDef {"
            f" id: {rust_str(ID + ':' + key)}, is_block: {str(is_block).lower()}, source: {rust_str(source)},"
            f" variants: &[{', '.join(rust_str(v) for v in variants)}], name: {name_rs},"
            f" name_color: 0x{color(m['name_color']):06X}, scale: {float(display.get('scale', 1.0))!r},"
            f" offset: [{float(display.get('x_offset', 0.0))!r}, {float(display.get('y_offset', 0.0))!r}, {float(display.get('z_offset', 0.0))!r}],"
            f" sim_cost: {int(m['sim_cost'])}, input: {rust_str(m['input'] if isinstance(m['input'], str) else m['input'].get('tag', m['input'].get('item', '')))},"
            f" base_drop: ({rust_str(base['id'])}, {int(base.get('count', 1))}), trivia: {rust_str(m['trivia'])},"
            f" fab: &[{', '.join(drops)}], required: {per_tier(m.get('required_data'))}, gained: {per_tier(gained)},"
            f" needs: &[{', '.join(rust_str(n) for n in needs)}], block_condition: {str(block_cond).lower()},"
            f" not_silk_touch: {str(not_silk).lower()} }},"
        )
    out.append("];")
    return "\n".join(out) + "\n"


def main(jar):
    source = zipfile.ZipFile(jar)
    files = {}
    models, tiers = {}, {}
    skipped = []
    for name in source.namelist():
        if name.endswith("/") or not name.startswith(("assets/", "data/")):
            continue
        if name.startswith("data/curios/") or name.startswith(f"data/{ID}/curios/"):
            continue
        data = source.read(name)
        if name.startswith(f"data/{ID}/{ID}/data_models/") and name.endswith(".json"):
            models[name[len(f"data/{ID}/{ID}/data_models/"):-5]] = json.loads(data.decode("utf-8-sig"))
            continue
        if name.startswith(f"data/{ID}/{ID}/model_tiers/") and name.endswith(".json"):
            tiers[name[len(f"data/{ID}/{ID}/model_tiers/"):-5]] = json.loads(data.decode("utf-8-sig"))
            continue
        if name.startswith(f"data/{ID}/recipe/") and name.endswith(".json"):
            doc = json.loads(data.decode("utf-8-sig"))
            needs = [c.get("modid") for c in doc.get("neoforge:conditions", []) if c.get("type") == "neoforge:mod_loaded"]
            if any(n not in ("minecraft", ID) for n in needs):
                skipped.append(name)
                continue
            doc.pop("neoforge:conditions", None)
            data = json.dumps(doc, indent=2).encode()
        files[name] = data
        if name.startswith(f"assets/{ID}/textures/jei/"):
            # JEI's card backgrounds, where GUI sprites are looked up (`hostilenetworks:gui/jei/...`).
            files[name.replace("/textures/jei/", "/textures/gui/jei/")] = data
    for name, (h, r) in MACHINES.items():
        files[f"data/{ID}/horizon/block/{name}.json"] = json.dumps(machine(name), indent=2).encode()
    files[f"data/{ID}/horizon/block/data_center_io_port.json"] = json.dumps(io_port(), indent=2).encode()
    for name, stack in ITEMS.items():
        decl = {"template": "minecraft:paper"}
        if stack != 64:
            decl["max_stack_size"] = stack
        files[f"data/{ID}/horizon/item/{name}.json"] = json.dumps(decl, indent=2).encode()
    for name, menu in MENUS.items():
        files[f"data/{ID}/horizon/menu/{name}.json"] = json.dumps(menu, indent=2).encode()
    for tag, values in C_TAGS.items():
        files[f"data/c/tags/item/{tag}.json"] = json.dumps({"replace": False, "values": values}, indent=2).encode()
    # The Data Center's floor tag (#c:obsidians) is a block tag too.
    files["data/c/tags/block/obsidians.json"] = json.dumps({"replace": False, "values": C_TAGS["obsidians"]}, indent=2).encode()
    # Hostile.Tabs: the blocks and items; the data models and predictions of every model are added in code
    # (DataModelItem and MobPredictionItem fill the tab).
    files[f"data/{ID}/horizon/creative_tab/tab.json"] = json.dumps({
        "title": "itemGroup.hostilenetworks",
        "icon": f"{ID}:sim_chamber",
        "items": [f"{ID}:{n}" for n in ("blank_data_model", "prediction_matrix", "overworld_prediction", "nether_prediction",
                                          "end_prediction", "twilight_prediction", "deep_learner", "sim_chamber",
                                          "loot_fabricator", "data_center", "data_center_io_port", "fab_directive")],
    }, indent=2).encode()
    sys.path.insert(0, HERE)
    import guide
    guide.write(files)
    lang = f"assets/{ID}/lang/en_us.json"
    doc = json.loads(files[lang].decode("utf-8-sig"))
    doc[f"{ID}.guide.title"] = "Hostile Neural Networks"
    files[lang] = json.dumps(doc, indent=2, ensure_ascii=False).encode("utf8")
    for tree in ("assets", "data"):
        shutil.rmtree(os.path.join(ROOT, tree), ignore_errors=True)
    for name, data in files.items():
        # Sunrise reads a content mod's blocks, items, menus and tabs from data/<id>/sunrise/ (Horizon's was horizon/).
        name = name.replace(f"data/{ID}/horizon/", f"data/{ID}/sunrise/", 1)
        path = os.path.join(ROOT, *name.split("/"))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write(data)
    with open(os.path.join(ROOT, "src", "models_data.rs"), "w", encoding="utf8", newline="\n") as f:
        f.write(models_source(models, tiers))
    print(len(files), "files,", len(models), "models,", len(tiers), "tiers; left out:", ", ".join(skipped) or "none")


def commands_menu():
    """The `/hostilenetworks` commands menu's pages (`../../tools/commands-menu/hostilenetworks.json`), written into assets/ after the wipe."""
    import importlib.util
    path = os.path.join(ROOT, "..", "tools", "commands-menu", "pages.py")
    spec = importlib.util.spec_from_file_location("commands_menu_pages", path)
    pages = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(pages)
    pages.install(ROOT, "hostilenetworks")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else JAR)
    commands_menu()
