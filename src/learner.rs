//! The Deep Learner: `DeepLearnerItem`, `DeepLearnerMenu`, `DeepLearnerScreen`, `DeepLearnerHudRenderer`,
//! `DataModelTooltipRenderer` and HostileEvents' kill and mine.
//!
//! Its four models live on the stack in `hostilenetworks:learner_inv` (ItemContainerContents). The fork opens a mod
//! menu only for a block entity, so the screen is the mod's own (the client draws it) and its clicks are requests the
//! server checks against the learner in the player's hand before it moves anything (`hostilenetworks:learner`).

use crate::config;
use crate::gui::*;
use crate::models::{self, Instance};
use crate::models_data::TIERS;
use crate::util::*;
use mod_api::*;
use std::sync::Mutex;

pub(crate) const LEARNER: &str = "hostilenetworks:deep_learner";
pub(crate) const NET_LEARNER: &str = "hostilenetworks:learner";
const W: f32 = 338.0;
const H: f32 = 235.0;
/// DeepLearnerMenu's slots, from the screen's corner.
const SLOTS: [[f32; 2]; 4] = [[256.0, 99.0], [274.0, 99.0], [256.0, 117.0], [274.0, 117.0]];

// ---- The learner's inventory (ItemContainerContents)

pub(crate) fn models_of(host: &dyn Host, learner: &ItemStack) -> [Option<ItemStack>; 4] {
    let mut out: [Option<ItemStack>; 4] = Default::default();
    if let Some(Tag::List(list)) = models::component(learner, models::LEARNER_INV) {
        for entry in list {
            let Tag::Compound(c) = entry else { continue };
            let slot = get_i(&c, "slot", -1);
            if !(0..4).contains(&slot) {
                continue;
            }
            if let Some(item) = get_c(&c, "item") {
                out[slot as usize] = ItemStack::from_nbt(host, item);
            }
        }
    }
    out
}

pub(crate) fn set_models(host: &dyn Host, learner: &mut ItemStack, inv: &[Option<ItemStack>; 4]) {
    let list: Vec<Tag> = inv
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let item = s.as_ref()?.to_nbt(host)?;
            let mut c = Compound::new();
            c.insert(java("slot"), Tag::Int(i as i32));
            c.insert(java("item"), Tag::Compound(item));
            Some(Tag::Compound(c))
        })
        .collect();
    if list.is_empty() {
        learner.remove(&models::Raw(models::LEARNER_INV));
    } else {
        models::set_component(learner, models::LEARNER_INV, Tag::List(list));
    }
}

// ---- HostileEvents.kill and mine

/// The learners a player carries (getNonEquipmentItems, then the off hand), by `ItemPlace::PlayerSlot` number.
fn learner_slots(host: &mut dyn ServerHost, player: u128) -> Vec<(i32, ItemStack)> {
    let mut out = Vec::new();
    for slot in (0..36).chain([-106]) {
        let place = ItemPlace::PlayerSlot { player, slot };
        if let Some(s) = place.with_stack(host, |s| s.project()).flatten()
            && is(&*host, &s, LEARNER)
        {
            out.push((slot, s));
        }
    }
    out
}

/// Replaces the stack at a player's slot by `stack` (the same item, its components changed).
fn rewrite(host: &mut dyn ServerHost, player: u128, slot: i32, old: &ItemStack, stack: ItemStack) -> bool {
    let place = ItemPlace::PlayerSlot { player, slot };
    let done = host.transaction(|host, tx| {
        let out = place.extract(host, tx, &ItemResource::of(old), old.count()).unwrap_or(0);
        if out != old.count() {
            return Err(());
        }
        let back = place.insert(host, tx, &ItemResource::of(&stack), stack.count()).unwrap_or(0);
        if back == stack.count() { Ok(()) } else { Err(()) }
    });
    done.is_ok()
}

/// updateModels: every model in the learner `matches` takes its tier's data per action.
fn update(host: &mut dyn ServerHost, player: u128, matches: &dyn Fn(&models::Model) -> bool) {
    for (slot, learner) in learner_slots(host, player) {
        let mut inv = models_of(&*host, &learner);
        let mut changed = false;
        for m in inv.iter_mut().flatten() {
            let Some(inst) = Instance::of(&*host, m) else { continue };
            if !matches(inst.model) {
                continue;
            }
            let gained = inst.model.gained(inst.tier);
            models::set_data(m, inst.data + gained);
            changed = true;
        }
        if changed {
            let mut new = learner.clone();
            set_models(&*host, &mut new, &inv);
            rewrite(host, player, slot, &learner, new);
        }
    }
}

/// HostileEvents.kill: the mob's killer (its kill credit, a player) trains the matching models of every learner they
/// carry.
pub(crate) fn killed(host: &mut dyn ServerHost, player: u128, kind: &str) {
    if !config::get(&*host).action_upgrades_model {
        return;
    }
    update(host, player, &|m: &models::Model| !m.def.is_block && m.matches(kind));
}

/// A kill counted once, whichever hook tells it first (the drops' kill credit, or the melee blow).
pub(crate) fn killed_once(host: &mut dyn ServerHost, player: u128, entity: u32, kind: &str) {
    static SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    {
        let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
        if seen.contains(&entity) {
            return;
        }
        seen.push(entity);
        if seen.len() > 64 {
            seen.remove(0);
        }
    }
    killed(host, player, kind);
}

/// HostileEvents.mine: and a block broken by a player, with the block model's upgrade conditions (not with Silk
/// Touch, for the ores).
pub(crate) fn mined(host: &mut dyn ServerHost, block: &str) {
    if !config::get(&*host).action_upgrades_model {
        return;
    }
    let Some(player) = host.players().acting_player() else { return };
    let uuid = player.uuid();
    let tool = ItemPlace::PlayerHand { player: uuid, hand: Hand::Main }.with_stack(host, |s| s.project()).flatten();
    let silk = tool.as_ref().is_some_and(|t| has_enchantment(t, "minecraft:silk_touch"));
    update(host, uuid, &|m: &models::Model| m.def.is_block && m.matches(block) && !(m.def.not_silk_touch && silk));
}

fn has_enchantment(stack: &ItemStack, id: &str) -> bool {
    let Some(Tag::Compound(enchants)) = stack.get(&models::Raw("minecraft:enchantments")) else { return false };
    let levels = match enchants.get(&java("levels")) {
        Some(Tag::Compound(l)) => l.clone(),
        _ => enchants,
    };
    levels.keys().any(|k| k.to_string_lossy() == id)
}

// ---- The screen's requests (DeepLearnerMenu's clicks), checked on the server

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    /// Inventory slot n (0..36) into the learner's first free slot.
    Put(i32),
    /// The learner's model n (0..4) out into the inventory.
    Take(usize),
}

pub(crate) fn encode(op: Op) -> Vec<u8> {
    match op {
        Op::Put(n) => format!("put {n}"),
        Op::Take(n) => format!("take {n}"),
    }
    .into_bytes()
}

pub(crate) fn decode(data: &[u8]) -> Option<Op> {
    if data.len() > 16 {
        return None;
    }
    let text = std::str::from_utf8(data).ok()?;
    let (verb, n) = text.split_once(' ')?;
    match (verb, n.parse::<i64>().ok()?) {
        ("put", n @ 0..=35) => Some(Op::Put(n as i32)),
        ("take", n @ 0..=3) => Some(Op::Take(n as usize)),
        _ => None,
    }
}

/// The learner a player holds: main hand (its hotbar slot, locked in the menu) else the off hand (DeepLearnerSource).
fn held(host: &mut dyn ServerHost, player: u128) -> Option<(i32, ItemStack)> {
    let p = host.player(player)?;
    let selected = p.selected_slot(&*host).unwrap_or(0) as i32;
    for slot in [selected, -106] {
        let s = ItemPlace::PlayerSlot { player, slot }.with_stack(host, |s| s.project()).flatten();
        if let Some(s) = s.filter(|s| is(&*host, s, LEARNER)) {
            return Some((slot, s));
        }
    }
    None
}

/// A request from `player`'s learner screen, done if what it names is still there.
pub(crate) fn request(host: &mut dyn ServerHost, player: u128, op: Op) {
    let Some((slot, learner)) = held(host, player) else { return };
    let mut inv = models_of(&*host, &learner);
    match op {
        Op::Put(from) => {
            if from == slot {
                return;
            }
            let Some(free) = inv.iter().position(Option::is_none) else { return };
            let place = ItemPlace::PlayerSlot { player, slot: from };
            let Some(src) = place.with_stack(host, |s| s.project()).flatten().filter(|s| is(&*host, s, "hostilenetworks:data_model")) else { return };
            let mut new_learner = learner.clone();
            let one = with_count(&src, 1);
            inv[free] = one;
            set_models(&*host, &mut new_learner, &inv);
            let _ = host.transaction(|host, tx| {
                if place.extract(host, tx, &ItemResource::of(&src), 1).unwrap_or(0) != 1 {
                    return Err(());
                }
                let lp = ItemPlace::PlayerSlot { player, slot };
                if lp.extract(host, tx, &ItemResource::of(&learner), 1).unwrap_or(0) != 1 || lp.insert(host, tx, &ItemResource::of(&new_learner), 1).unwrap_or(0) != 1 {
                    return Err(());
                }
                Ok(())
            });
        }
        Op::Take(i) => {
            let Some(model) = inv[i].take() else { return };
            let mut new_learner = learner.clone();
            set_models(&*host, &mut new_learner, &inv);
            // Into the inventory as a shift click puts it (the hotbar first, then the main slots); it stays in the
            // learner when there's no room.
            let _ = host.transaction(|host, tx| {
                let lp = ItemPlace::PlayerSlot { player, slot };
                if lp.extract(host, tx, &ItemResource::of(&learner), 1).unwrap_or(0) != 1 || lp.insert(host, tx, &ItemResource::of(&new_learner), 1).unwrap_or(0) != 1 {
                    return Err(());
                }
                for to in (0..9).chain(9..36) {
                    if to == slot {
                        continue;
                    }
                    let place = ItemPlace::PlayerSlot { player, slot: to };
                    if place.with_stack(host, |_| ()).is_none() {
                        return if place.insert(host, tx, &ItemResource::of(&model), 1).unwrap_or(0) == 1 { Ok(()) } else { Err(()) };
                    }
                }
                Err(())
            });
        }
    }
}

// ---- The screen (client)

#[derive(Default)]
pub(crate) struct View {
    selected: usize,
    spin: i32,
    last_tick: i64,
    ticks_shown: i32,
    variant: usize,
    /// The model last set up (setupModel), to restart its text when it changes.
    shown: Option<String>,
    main: Typewriter,
    info: Typewriter,
    data: Typewriter,
    empty: bool,
    origin: [f32; 2],
    pending: Vec<Op>,
}

static VIEW: Mutex<Option<View>> = Mutex::new(None);

fn with_view<T>(f: impl FnOnce(&mut View) -> T) -> T {
    let mut g = VIEW.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(View::default))
}

/// The local player's inventory by slot, as `Inventory` numbers it: 0..36 the main inventory, 36..40 the armor, 40
/// the offhand. None for a slot that is empty or can't be read.
fn inventory_by_slot(host: &dyn ClientHost, p: PlayerRef) -> Option<Vec<Option<ItemStack>>> {
    p.with_inventory(host, |view| (0..41).map(|n| view.get(n).and_then(|s| s.project())).collect())
}

/// The stack in `slot` of an inventory read by `inventory_by_slot`.
fn in_slot(inv: &[Option<ItemStack>], slot: usize) -> Option<&ItemStack> {
    inv.get(slot).and_then(Option::as_ref)
}

/// The learner in the local player's hand: its slot (as the server numbers it) and stack.
pub(crate) fn local_learner(host: &dyn ClientHost) -> Option<(i32, ItemStack)> {
    let p = host.local_player()?;
    let inv = inventory_by_slot(host, p)?;
    let selected = p.selected_slot(host).unwrap_or(0);
    if let Some(s) = in_slot(&inv, selected).filter(|s| is(host, s, LEARNER)) {
        return Some((selected as i32, s.clone()));
    }
    in_slot(&inv, 40).filter(|s| is(host, s, LEARNER)).map(|s| (-106, s.clone()))
}

/// DeepLearnerItem.use: the screen opens (on the using client).
pub(crate) fn open(host: &mut dyn ClientHost) {
    if host.mod_screen_open(NS) {
        *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some(View { empty: true, spin: 65, ..View::default() });
    }
}

/// Each client tick: requests made since go to the server.
pub(crate) fn client_tick(host: &mut dyn ClientHost) {
    let ops = with_view(|v| std::mem::take(&mut v.pending));
    for op in ops {
        let _ = host.send_payload_to_server(&PayloadId(NET_LEARNER.into()), PayloadBytes(encode(op)));
    }
}

fn setup(host: &dyn ClientHost, v: &mut View, inst: &Instance) {
    let m = inst.model;
    v.ticks_shown = 0;
    v.variant = 0;
    v.main.clear();
    v.info.clear();
    v.data.clear();
    v.main.add_line(lang(host, "hostilenetworks.gui.name", "Name"), BLUE, 1.0);
    v.main.add_line(if m.def.is_block { block_name(host, m.def.source) } else { entity_name(host, m.def.source) }, 0xFFFF_FFFF, 1.0);
    v.info.add_line(lang(host, "hostilenetworks.gui.info", "Information"), BLUE, 1.0);
    v.info.add_line(lang(host, m.def.trivia, ""), 0xFFFF_FFFF, 1.0);
    let tier = inst.tier;
    let next = models::next_tier(tier);
    let tier_line = lang(host, "hostilenetworks.gui.tier", "Model Tier: %s");
    let (a, b) = tier_line.split_once("%s").unwrap_or((&tier_line, ""));
    v.data.add_line(a.to_owned(), 0xFFFF_FFFF, 1.0);
    v.data.continue_line(models::tier_name(host, tier), argb(TIERS[tier].color));
    v.data.continue_line(b.to_owned(), 0xFFFF_FFFF);
    let acc_line = lang(host, "hostilenetworks.gui.accuracy", "Model Accuracy: %s");
    let (a, b) = acc_line.split_once("%s").unwrap_or((&acc_line, ""));
    v.data.add_line(a.to_owned(), 0xFFFF_FFFF, 1.0);
    v.data.continue_line(format!("{}%", crate::sim::format_accuracy(100.0 * inst.accuracy(host))), argb(TIERS[tier].color));
    v.data.continue_line(b.to_owned(), 0xFFFF_FFFF);
    if !inst.is_max() {
        if config::get(host).action_upgrades_model {
            let actions = inst.actions_needed(host);
            let word = if m.def.is_block { "hostilenetworks.gui.action.mine" } else { "hostilenetworks.gui.action.kill" };
            let word = lang(host, &if actions > 1 { format!("{word}s") } else { word.to_owned() }, "");
            let line = lang(host, "hostilenetworks.gui.next_tier", "Upgrades to %s in %s %s");
            let mut parts = line.splitn(2, "%s");
            let first = parts.next().unwrap_or("").to_owned();
            let rest = parts.next().unwrap_or("");
            v.data.add_line(first, 0xFFFF_FFFF, 1.0);
            v.data.continue_line(models::tier_name(host, next), argb(TIERS[next].color));
            v.data.continue_line(format_args_java(rest, &[actions.to_string(), word]), 0xFFFF_FFFF);
        } else {
            v.data.add_line(lang(host, "hostilenetworks.gui.upgrade_disabled", ""), 0xFFFF_FFFF, 1.0);
        }
    } else {
        v.data.add_line(lang(host, "hostilenetworks.gui.max_tier", "You cannot improve me further"), argb(0xFF5555), 1.0);
    }
}

fn setup_empty(host: &dyn ClientHost, v: &mut View) {
    v.main.clear();
    v.info.clear();
    v.data.clear();
    for i in 0..7 {
        v.main.add_line(lang(host, &format!("hostilenetworks.gui.learner_empty.{i}"), ""), if i == 0 { BLUE } else { 0xFFFF_FFFF }, 1.0);
    }
    v.empty = true;
    v.shown = None;
}

/// EntityDataModel.getStatistics: health and armour halved, and the experience a kill gives.
fn statistics(host: &dyn ClientHost, m: &models::Model) -> [String; 3] {
    if m.def.is_block {
        let state = host.block_state_id(m.def.source);
        let hardness = state.and_then(|s| host.block_states().destroy_speed(s)).unwrap_or(0.0);
        let sound = state.and_then(|s| host.block_states().sound(s)).map(|s| format!("{:?}", s)).unwrap_or_default();
        return [crate::sim::format_accuracy(hardness), "-".to_owned(), sound];
    }
    match crate::mobs::stats(m.def.source) {
        Some((health, armor, xp)) => [((health / 2.0) as i32).to_string(), ((armor / 2.0) as i32).to_string(), xp.to_string()],
        None => ["?".to_owned(), "?".to_owned(), "?".to_owned()],
    }
}

/// Draws the screen over the world (the mod screen's `screen_render_post`).
pub(crate) fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, mouse: [f32; 2]) {
    let Some((held_slot, learner)) = local_learner(host) else { return };
    let [sw, sh] = draw.size();
    let o = [((sw - W) / 2.0).floor(), ((sh - H) / 2.0).floor()];
    let inv = models_of(host, &learner);
    let insts: Vec<Option<Instance>> = inv.iter().map(|s| s.as_ref().and_then(|s| Instance::of(host, s))).collect();
    let count = insts.iter().flatten().count();
    let now = host.level(host.dimension()).and_then(|l| l.game_time(host)).unwrap_or(0);
    draw.draw_menu_background([0.0, 0.0, sw, sh], true);
    let tex = "hostilenetworks:gui/deep_learner";
    blit(draw, tex, o, 41.0, 0.0, 0.0, 0.0, 256.0, 140.0);
    let selected = with_view(|v| {
        v.origin = o;
        if insts.get(v.selected).is_none_or(Option::is_none) {
            v.selected = insts.iter().position(Option::is_some).unwrap_or(0);
        }
        // containerTick.
        if v.last_tick != now {
            v.last_tick = now;
            match &insts[v.selected] {
                None => {
                    if !v.empty {
                        setup_empty(host, v);
                    }
                    if v.main.is_empty() {
                        setup_empty(host, v);
                    }
                }
                Some(inst) => {
                    let key = format!("{}|{}|{}", v.selected, inst.model.id(), inst.data);
                    if v.shown.as_deref() != Some(key.as_str()) {
                        setup(host, v, inst);
                        v.shown = Some(key);
                        v.empty = false;
                    }
                }
            }
            v.main.tick();
            v.info.tick();
            v.data.tick();
            v.spin += 1;
            v.ticks_shown += 1;
        }
        v.selected
    });
    let stats_title = lang(host, "hostilenetworks.gui.stats", "Statistics");
    let stats_w = draw.text_width(&stats_title).max(0.0);
    if count > 0
        && let Some(inst) = &insts[selected]
    {
        let column = if inst.model.def.is_block { 96.0 } else { 0.0 };
        for i in 0..3 {
            blit(draw, tex, o, W - 49.0 - stats_w, 8.0 + 9.0 + 11.0 * i as f32, column, 140.0 + 9.0 * i as f32, 9.0, 9.0);
        }
        blit(draw, tex, o, -41.0, 0.0, 9.0, 140.0, 75.0, 101.0);
        let spin = with_view(|v| v.spin) as f32;
        crate::mobs::draw_mob(host, draw, inst.model, [o[0] - 4.0, o[1] + 90.0], 40.0, (spin * 2.25) % 360.0, [o[0] - 41.0, o[1], 75.0, 101.0]);
        let stats = statistics(host, inst.model);
        for (i, s) in stats.iter().enumerate() {
            draw.text(s, [o[0] + W - 36.0 - stats_w, o[1] + 9.0 + 9.0 + 11.0 * i as f32], 0xFFFF_FFFF, true);
        }
        draw.text(&stats_title, [o[0] + 289.0 - stats_w, o[1] + 6.0], BLUE, true);
    }
    blit(draw, "hostilenetworks:gui/default_gui", o, 81.0, 145.0, 0.0, 0.0, 176.0, 90.0);
    // The arrows (ImageButtons at -27,105 and -1,105).
    if count > 1 {
        for (x, name) in [(-27.0, "left"), (-1.0, "right")] {
            let hover = inside([o[0] + x, o[1] + 105.0, 24.0, 24.0], mouse);
            let sprite = format!("hostilenetworks:gui/sprites/widget/{name}{}", if hover { "_hovered" } else { "" });
            draw.sprite(&sprite, [0.0, 0.0, 24.0, 24.0], [o[0] + x, o[1] + 105.0, 24.0, 24.0], None);
        }
    }
    // extractLabels.
    with_view(|v| {
        let name_lines = 1;
        v.main.max_width = 200.0 - stats_w + 36.0;
        v.info.max_width = v.main.max_width;
        v.data.max_width = 200.0;
        let drawn = v.main.render(draw, [o[0] + 49.0, o[1] + 6.0]);
        let _ = drawn;
        v.info.render(draw, [o[0] + 49.0, o[1] + 6.0 + 12.0 * (1 + name_lines) as f32]);
        v.data.render(draw, [o[0] + 49.0, o[1] + 6.0 + 12.0 * 8.0]);
    });
    // The slots: the learner's four, then the inventory (the learner in hand is locked).
    let player_inv = host.local_player().and_then(|p| inventory_by_slot(host, p)).unwrap_or_default();
    let mut hovered: Option<ItemStack> = None;
    let mut slot = |draw: &mut dyn DrawContext, p: [f32; 2], stack: Option<&ItemStack>, locked: bool| {
        let r = [o[0] + p[0], o[1] + p[1], 16.0, 16.0];
        if let Some(s) = stack {
            crate::fab::draw_stack(draw, s, [r[0], r[1]]);
        }
        if !locked && inside(r, mouse) {
            draw.fill(r, 0x80FF_FFFF);
            hovered = stack.cloned();
        }
    };
    for (i, p) in SLOTS.iter().enumerate() {
        if inv[i].is_none() {
            draw.sprite("hostilenetworks:item/empty_learner_slot", [0.0, 0.0, 16.0, 16.0], [o[0] + p[0], o[1] + p[1], 16.0, 16.0], None);
        }
        slot(draw, *p, inv[i].as_ref(), false);
    }
    for (n, p) in inventory_slots() {
        slot(draw, p, in_slot(&player_inv, n as usize), n == held_slot);
    }
    if let Some(s) = hovered {
        let lines = crate::tooltips::lines(host, &s);
        let texts: Vec<Text> = lines.iter().map(|l| legacy_text(l)).collect();
        let _ = draw.gui_tooltip(&texts, mouse);
    }
}

/// The inventory slots the screen shows: (inventory slot, corner from the screen's).
fn inventory_slots() -> impl Iterator<Item = (i32, [f32; 2])> {
    let main = (0..27).map(|i| (9 + i, [89.0 + (i % 9) as f32 * 18.0, 153.0 + (i / 9) as f32 * 18.0]));
    let bar = (0..9).map(|i| (i, [89.0 + i as f32 * 18.0, 211.0]));
    main.chain(bar)
}

/// A press on the screen: the arrows choose the model shown; a model in the inventory goes in, one in the learner
/// comes out. True when the screen took it.
pub(crate) fn press(host: &dyn ClientHost, mouse: [f32; 2], button: i32) -> bool {
    if button != 0 {
        return true;
    }
    let Some((held_slot, learner)) = local_learner(host) else { return true };
    let inv = models_of(host, &learner);
    let valid: Vec<bool> = inv.iter().map(|s| s.as_ref().is_some_and(|s| Instance::of(host, s).is_some())).collect();
    let count = valid.iter().filter(|v| **v).count();
    let o = with_view(|v| v.origin);
    let at = [mouse[0] - o[0], mouse[1] - o[1]];
    let step = |d: i32| {
        with_view(|v| {
            for k in 1..=4 {
                let i = (v.selected as i32 + d * k).rem_euclid(4) as usize;
                if valid[i] {
                    v.selected = i;
                    v.shown = None;
                    break;
                }
            }
        })
    };
    if count > 1 && inside([-27.0, 105.0, 24.0, 24.0], at) {
        step(-1);
        return true;
    }
    if count > 1 && inside([-1.0, 105.0, 24.0, 24.0], at) {
        step(1);
        return true;
    }
    for (i, p) in SLOTS.iter().enumerate() {
        if inside([p[0] - 1.0, p[1] - 1.0, 18.0, 18.0], at) && inv[i].is_some() {
            with_view(|v| v.pending.push(Op::Take(i)));
            return true;
        }
    }
    let player_inv = host.local_player().and_then(|p| inventory_by_slot(host, p)).unwrap_or_default();
    for (n, p) in inventory_slots() {
        if n != held_slot
            && inside([p[0] - 1.0, p[1] - 1.0, 18.0, 18.0], at)
            && in_slot(&player_inv, n as usize).is_some_and(|s| is(host, s, "hostilenetworks:data_model"))
        {
            with_view(|v| v.pending.push(Op::Put(n)));
            return true;
        }
    }
    true
}

// ---- The HUD (DeepLearnerHudRenderer) and the tooltip's model bars (DataModelTooltipRenderer)

fn hud_models(host: &dyn ClientHost, learner: &ItemStack) -> Vec<(ItemStack, Instance)> {
    models_of(host, learner).into_iter().flatten().filter_map(|s| Instance::of(host, &s).map(|i| (s, i))).collect()
}

/// A model's bar and text, as the HUD and the tooltip draw them, at `at` (x, y of the row).
fn model_row(host: &dyn ClientHost, draw: &mut dyn DrawContext, stack: &ItemStack, inst: &Instance, at: [f32; 2], text_dx: f32, bar: [f32; 2], kills: [f32; 2]) {
    draw_row(draw, &Row::of(host, stack, inst), at, text_dx, bar, kills);
}

/// One model row's look, read on the client ahead of drawing (the tooltip component draws without a host).
#[derive(Clone)]
pub(crate) struct Row {
    stack: ItemStack,
    width: f32,
    tier: String,
    colour: u32,
    model_word: String,
    left: Option<String>,
}

impl Row {
    fn of(host: &dyn ClientHost, stack: &ItemStack, inst: &Instance) -> Self {
        let width = if inst.is_max() {
            87.0
        } else {
            let prev = inst.tier_data();
            (87.0 * (inst.data - prev) as f32 / (inst.next_tier_data() - prev).max(1) as f32).ceil()
        };
        Row {
            stack: stack.clone(),
            width,
            tier: models::tier_name(host, inst.tier),
            colour: argb(TIERS[inst.tier].color),
            model_word: lang(host, "hostilenetworks.hud.model", " Model"),
            left: (!inst.is_max()).then(|| lang_args(host, "hostilenetworks.hud.kills", "%s Remaining", &[inst.actions_needed(host).to_string()])),
        }
    }
}

fn draw_row(draw: &mut dyn DrawContext, row: &Row, at: [f32; 2], text_dx: f32, bar: [f32; 2], kills: [f32; 2]) {
    let hud = "hostilenetworks:gui/deep_learner_hud";
    draw.sprite(hud, [0.0, 0.0, 89.0, 12.0], [at[0] + bar[0], at[1] + bar[1], 89.0, 12.0], None);
    let width = row.width;
    draw.sprite(hud, [0.0, 12.0, width, 10.0], [at[0] + bar[0] + 1.0, at[1] + bar[1] + 1.0, width, 10.0], None);
    draw.item_stack(&row.stack, [at[0], at[1] + bar[1] - 2.0], 16.0);
    let tier = &row.tier;
    let colour = row.colour;
    draw.text(tier, [at[0] + text_dx, at[1]], colour, true);
    draw.text(&row.model_word, [at[0] + text_dx + draw.text_width(tier), at[1]], colour, true);
    if let Some(left) = &row.left {
        draw.text(left, [at[0] + kills[0], at[1] + kills[1]], 0xFFFF_FFFF, true);
    }
}

pub(crate) fn hud(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
    if host.mod_screen().is_some() {
        return;
    }
    let Some(p) = host.local_player() else { return };
    let inv = inventory_by_slot(host, p).unwrap_or_default();
    let selected = p.selected_slot(host).unwrap_or(0);
    let stack = in_slot(&inv, selected).filter(|s| is(host, s, LEARNER)).or_else(|| in_slot(&inv, 40).filter(|s| is(host, s, LEARNER)));
    let Some(stack) = stack else { return };
    let rows = hud_models(host, stack);
    if rows.is_empty() {
        return;
    }
    let cfg = config::get(host);
    let [sw, sh] = draw.size();
    let (ew, eh) = (119.0, 10.0 + 28.0 * rows.len() as f32);
    // Placebo's Offset: an anchor of the window and of the element, then the offset.
    let (fx, fy) = match cfg.hud_anchor.as_str() {
        "top_center" => (0.5, 0.0),
        "top_right" => (1.0, 0.0),
        "center_left" => (0.0, 0.5),
        "center" => (0.5, 0.5),
        "center_right" => (1.0, 0.5),
        "bottom_left" => (0.0, 1.0),
        "bottom_center" => (0.5, 1.0),
        "bottom_right" => (1.0, 1.0),
        _ => (0.0, 0.0),
    };
    let base = [(sw - ew) * fx + cfg.hud_x as f32, (sh - eh) * fy + cfg.hud_y as f32];
    draw.gui_nine_slice("hostilenetworks:dl_hud_bg", [base[0] + 3.0, base[1] + 3.0, 113.0, 5.0 + 28.0 * rows.len() as f32], 0xFFFF_FFFF);
    for (i, (s, inst)) in rows.iter().enumerate() {
        let at = [base[0] + 6.0, base[1] + 6.0 + 28.0 * i as f32];
        model_row(host, draw, s, inst, at, 2.0, [18.0, 11.0], [21.0, 13.0]);
    }
}

/// The learner's tooltip image: each model's bar (29 high, at least 107 wide).
/// `getTooltipImage`: the learner's models, read now, as typed tooltip data.
pub(crate) fn tooltip_image(host: &dyn ClientHost, stack: &ItemStack) -> Option<TooltipComponent> {
    let rows = hud_models(host, stack);
    if rows.is_empty() {
        return None;
    }
    let width = rows.iter().map(|(_, i)| {
        let t = models::tier_name(host, i.tier) + &lang(host, "hostilenetworks.hud.model", " Model");
        host.text_width(&Text::literal(t)) as f32
    });
    let w = width.fold(107.0, f32::max);
    let size = [w as i32, 29 * rows.len() as i32];
    let rows = rows.iter().map(|(s, inst)| Row::of(host, s, inst)).collect();
    Some(TooltipComponent::custom(LearnerTooltip { size, rows }))
}

/// The learner's tooltip data (`DeepLearnerItem.getTooltipImage`'s component).
pub(crate) struct LearnerTooltip {
    size: [i32; 2],
    rows: Vec<Row>,
}

impl TooltipComponentData for LearnerTooltip {}

/// Its client component: each model's bar (29 high, at least 107 wide).
pub(crate) struct LearnerTooltipClient {
    size: [i32; 2],
    rows: Vec<Row>,
}

impl LearnerTooltipClient {
    pub(crate) fn new(data: &LearnerTooltip) -> Self {
        LearnerTooltipClient { size: data.size, rows: data.rows.clone() }
    }
}

impl ClientTooltipComponent for LearnerTooltipClient {
    fn height(&self, _font: &dyn TooltipFont) -> i32 {
        self.size[1]
    }

    fn width(&self, _font: &dyn TooltipFont) -> i32 {
        self.size[0]
    }

    fn draw_image(&self, _font: &dyn TooltipFont, x: i32, y: i32, _width: i32, _height: i32, draw: &mut dyn DrawContext) {
        for (i, row) in self.rows.iter().enumerate() {
            let at = [x as f32, y as f32 + 29.0 * i as f32];
            draw_row(draw, row, at, 0.0, [20.0, 12.0], [23.0, 14.0]);
        }
    }
}
