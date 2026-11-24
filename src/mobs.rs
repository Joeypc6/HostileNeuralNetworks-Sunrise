//! The mobs data models show: their statistics (EntityDataModel.getStatistics) and their picture.
//!
//! The mob is drawn as the Java mod draws its ghost (`DrawContext::entity`, vanilla's `renderEntityInInventory`);
//! a block model, or a mob the game can't draw, falls back to its block, spawn egg or head.

use crate::models::Model;
use crate::util::*;
use mod_api::*;

/// Vanilla's attributes of the mobs the jar has models for: (max health, armour, base experience reward).
/// Animals and water animals give 1 to 3 (1 + random(3)); shown as their middle, 2.
const STATS: &[(&str, f32, f32, i32)] = &[
    ("minecraft:bat", 6.0, 0.0, 0),
    ("minecraft:blaze", 20.0, 0.0, 10),
    ("minecraft:breeze", 30.0, 0.0, 10),
    ("minecraft:chicken", 4.0, 0.0, 2),
    ("minecraft:cod", 3.0, 0.0, 2),
    ("minecraft:cow", 10.0, 0.0, 2),
    ("minecraft:creeper", 20.0, 0.0, 5),
    ("minecraft:drowned", 20.0, 2.0, 5),
    ("minecraft:elder_guardian", 80.0, 0.0, 10),
    ("minecraft:ender_dragon", 200.0, 0.0, 500),
    ("minecraft:enderman", 40.0, 0.0, 5),
    ("minecraft:evoker", 24.0, 0.0, 10),
    ("minecraft:ghast", 10.0, 0.0, 5),
    ("minecraft:glow_squid", 10.0, 0.0, 2),
    ("minecraft:guardian", 30.0, 0.0, 10),
    ("minecraft:hoglin", 40.0, 0.0, 5),
    ("minecraft:iron_golem", 100.0, 0.0, 0),
    ("minecraft:magma_cube", 16.0, 0.0, 4),
    ("minecraft:mooshroom", 10.0, 0.0, 2),
    ("minecraft:phantom", 20.0, 0.0, 5),
    ("minecraft:pig", 10.0, 0.0, 2),
    ("minecraft:polar_bear", 30.0, 0.0, 2),
    ("minecraft:rabbit", 3.0, 0.0, 2),
    ("minecraft:sheep", 8.0, 0.0, 2),
    ("minecraft:shulker", 30.0, 20.0, 5),
    ("minecraft:skeleton", 20.0, 0.0, 5),
    ("minecraft:slime", 16.0, 0.0, 4),
    ("minecraft:snow_golem", 4.0, 0.0, 0),
    ("minecraft:spider", 16.0, 0.0, 5),
    ("minecraft:squid", 10.0, 0.0, 2),
    ("minecraft:vindicator", 24.0, 0.0, 5),
    ("minecraft:warden", 500.0, 0.0, 5),
    ("minecraft:witch", 26.0, 0.0, 5),
    ("minecraft:wither", 300.0, 4.0, 50),
    ("minecraft:wither_skeleton", 20.0, 0.0, 5),
    ("minecraft:zombie", 20.0, 2.0, 5),
    ("minecraft:zombified_piglin", 20.0, 2.0, 5),
];

pub(crate) fn stats(kind: &str) -> Option<(f32, f32, i32)> {
    STATS.iter().find(|(k, ..)| *k == kind).map(|(_, h, a, x)| (*h, *a, *x))
}

/// The item that stands for a model's mob: the block, its spawn egg, else its head.
pub(crate) fn picture_item(host: &dyn Host, m: &Model) -> Option<ItemKey> {
    let source = m.def.source;
    if m.def.is_block {
        return item(host, source);
    }
    let (ns, path) = source.split_once(':')?;
    if path == "ender_dragon" {
        return item(host, "minecraft:dragon_head");
    }
    item(host, &format!("{ns}:{path}_spawn_egg")).or_else(|| item(host, &format!("{ns}:{path}_head")))
}

/// Vanilla's standing heights (EntityType sizes, blocks) of the mobs the jar has models for; others 1.8.
const HEIGHTS: &[(&str, f32)] = &[
    ("minecraft:bat", 0.9), ("minecraft:blaze", 1.8), ("minecraft:breeze", 1.77), ("minecraft:chicken", 0.7),
    ("minecraft:cod", 0.3), ("minecraft:cow", 1.4), ("minecraft:creeper", 1.7), ("minecraft:drowned", 1.95),
    ("minecraft:elder_guardian", 1.9975), ("minecraft:ender_dragon", 8.0), ("minecraft:enderman", 2.9),
    ("minecraft:evoker", 1.95), ("minecraft:ghast", 4.0), ("minecraft:glow_squid", 0.8), ("minecraft:guardian", 0.85),
    ("minecraft:hoglin", 1.4), ("minecraft:iron_golem", 2.7), ("minecraft:magma_cube", 0.52), ("minecraft:mooshroom", 1.4),
    ("minecraft:phantom", 0.5), ("minecraft:pig", 0.9), ("minecraft:polar_bear", 1.4), ("minecraft:rabbit", 0.5),
    ("minecraft:sheep", 1.3), ("minecraft:shulker", 1.0), ("minecraft:skeleton", 1.99), ("minecraft:slime", 0.52),
    ("minecraft:snow_golem", 1.9), ("minecraft:spider", 0.9), ("minecraft:squid", 0.8), ("minecraft:vindicator", 1.95),
    ("minecraft:warden", 2.9), ("minecraft:witch", 1.95), ("minecraft:wither", 3.5), ("minecraft:wither_skeleton", 2.4),
    ("minecraft:zombie", 1.95), ("minecraft:zombified_piglin", 1.95),
];

fn height(kind: &str) -> f32 {
    HEIGHTS.iter().find(|(k, _)| *k == kind).map_or(1.8, |(_, h)| *h)
}

/// The model's mob standing with its feet at `feet`, `size` GUI pixels a block (already times its display scale),
/// turned `yaw` degrees, drawn within `rect` (a big mob may reach past its slot, as the Java mod's does). False when
/// it is a block model or the game can't draw it.
pub(crate) fn draw_entity(draw: &mut dyn DrawContext, m: &Model, feet: [f32; 2], size: f32, yaw: f32, rect: [f32; 4]) -> bool {
    if m.def.is_block || size <= 0.0 {
        return false;
    }
    let kind = m.def.source;
    // DisplayEntity's x/y/z offsets, in the mob's own blocks: the Java mod draws the mob there (y up).
    let feet = [feet[0] + m.def.offset[0] * size, feet[1] - m.def.offset[1] * size];
    // DrawContext::entity centres the mob's middle in `rect` and moves it `offset_y` blocks down: feet at
    // centre + (height / 2 + offset_y) * size.
    let centre = (rect[1] + rect[3]) / 2.0;
    let offset_y = (feet[1] - centre) / size - height(kind) / 2.0;
    let rect = [feet[0] - (rect[2] - rect[0]) / 2.0, rect[1], feet[0] + (rect[2] - rect[0]) / 2.0, rect[3]];
    let view = GuiEntityView::new(rect, size, offset_y, GuiEntityLook::Turned { yaw, pitch: 0.0 });
    draw.entity(&GuiEntity::kind(kind), view).is_ok()
}

/// The model's mob with its feet at `feet`, `size` GUI pixels a block (times its display scale), kept in `clip`
/// (x, y, w, h): the mob itself, else its picture item.
pub(crate) fn draw_mob(host: &dyn ClientHost, draw: &mut dyn DrawContext, m: &Model, feet: [f32; 2], size: f32, yaw: f32, clip: [f32; 4]) {
    let px = size * m.def.scale.max(0.05);
    if draw_entity(draw, m, feet, px, yaw, [clip[0], clip[1], clip[0] + clip[2], clip[1] + clip[3]]) {
        return;
    }
    let Some(key) = picture_item(host, m) else { return };
    let s = (size * 0.8 * m.def.scale.max(0.2)).min(clip[2]).min(clip[3]).min(32.0);
    draw.item(key, [feet[0] - s / 2.0, feet[1] - s], s);
}
