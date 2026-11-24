//! The Simulation Chamber: `SimChamberTileEntity`, `SimChamberMenu` and `SimChamberScreen`.
//!
//! Slots: 0 the data model, 1 the input, 2 the base drop, 3 the prediction. Saved as the Java tile saves them
//! (`energy`, `model`, `runtime`, `predSuccess`, `failState`, `redstoneState`, `simMode`). Menu data: 0 runtime,
//! 1 prediction success, 2 fail state, 3 redstone state, 4 mode, 5-6 energy (low and high 16 bits).

use crate::config;
use crate::gui::*;
use crate::models::{self, Instance, TIER_ORDER};
use crate::models_data::TIERS;
use crate::util::*;
use mod_api::*;
use std::sync::Mutex;

pub(crate) const BLOCK: &str = "hostilenetworks:sim_chamber";
pub(crate) const DATA_SLOTS: usize = 7;

/// SimMode: INFERENCE (speed 1.25, cost 1.0) and TRAINING (speed 1.0, cost 1.2); runtime round(300 / speed).
pub(crate) fn runtime_of(mode: i32) -> i32 {
    if mode == 1 { 300 } else { 240 }
}

fn cost_mult(mode: i32) -> f32 {
    if mode == 1 { 1.2 } else { 1.0 }
}

/// SimMode.adjustCost: Mth.ceil(baseCost * costMultiplier).
pub(crate) fn adjust_cost(mode: i32, base: i32) -> i32 {
    (base as f32 * cost_mult(mode)).ceil() as i32
}

/// What a stack's model, data and iterations say: a model swapped by hand restarts the run (getOrLoadModel).
fn signature(stack: &ItemStack) -> String {
    format!("{}|{}|{}", models::stored_id(stack).unwrap_or_default(), models::data_of(stack), models::iters_of(stack))
}

/// canUpgrade.
fn can_upgrade(host: &dyn Host, inst: &Instance) -> bool {
    let cfg = config::get(host);
    if inst.is_max() || cfg.sim_model_upgrade == 0 {
        return false;
    }
    cfg.sim_model_upgrade != 2 || inst.data + 1 <= inst.next_tier_data()
}

/// SimChamberTileEntity.canStack: empty, or the same item and components below its max size.
fn can_stack(host: &dyn Host, slot: &Option<ItemStack>, expected: &ItemStack) -> bool {
    match slot {
        None => true,
        Some(a) => same(a, expected) && a.count() < max_stack(host, a),
    }
}

/// serverTick.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: Pos, e: &mut Fields) {
    let cfg = config::get(host);
    e.items.resize(4, None);
    if e.data.len() < DATA_SLOTS {
        e.data.resize(DATA_SLOTS, 0);
    }
    let nbt = &mut e.nbt;
    let mut mode = get_i(nbt, "simMode", 0);
    if mode == 1 && cfg.sim_model_upgrade == 0 {
        mode = 0;
        set_i(nbt, "simMode", 0);
    }
    let mut runtime = get_i(nbt, "runtime", 0);
    let mut success = get_i(nbt, "predSuccess", 0);
    let mut fail = get_i(nbt, "failState", F_NONE);
    let redstone = get_i(nbt, "redstoneState", 0);
    'run: {
        let Some(stack) = e.items[0].clone() else {
            fail = F_MODEL;
            runtime = 0;
            break 'run;
        };
        let sig = signature(&stack);
        if get_s(&e.nbt, "hz_model").as_deref() != Some(sig.as_str()) {
            runtime = 0;
        }
        let Some(inst) = Instance::of(host, &stack) else {
            fail = F_MODEL;
            runtime = 0;
            e.nbt.insert(java("hz_model"), Tag::String(sig.as_str().into()));
            break 'run;
        };
        e.nbt.insert(java("model"), Tag::String(inst.model.id().into()));
        e.nbt.insert(java("hz_model"), Tag::String(sig.as_str().into()));
        if !TIERS[inst.tier].can_sim {
            fail = F_FAULTY;
            runtime = 0;
            break 'run;
        }
        let cost = adjust_cost(mode, inst.model.sim_cost());
        let energy = get_i(&e.nbt, "energy", 0);
        if runtime == 0 {
            // canStartSimulation.
            let input_ok = e.items[1].as_ref().is_some_and(|s| inst.model.input_matches(host, s));
            if !input_ok {
                fail = F_INPUT;
                break 'run;
            }
            if !redstone_matches(redstone, powered(host, pos)) {
                fail = F_REDSTONE;
                break 'run;
            }
            if mode == 1 && !can_upgrade(host, &inst) {
                fail = F_CANNOT_TRAIN;
                break 'run;
            }
            let base = stack_of(host, inst.model.def.base_drop.0, inst.model.def.base_drop.1);
            let pred = models::prediction(host, inst.model, 1);
            let (Some(base), Some(pred)) = (base, pred) else { break 'run };
            if !can_stack(host, &e.items[2], &base) || !can_stack(host, &e.items[3], &pred) {
                fail = F_OUTPUT;
                break 'run;
            }
            if energy < cost {
                fail = F_ENERGY;
                break 'run;
            }
            fail = F_NONE;
            runtime = runtime_of(mode);
            success = inst.roll(host, pos);
            shrink_one(&mut e.items[1]);
        } else if energy >= cost {
            if redstone_matches(redstone, powered(host, pos)) {
                fail = F_NONE;
                runtime -= 1;
                if runtime == 0 {
                    if mode == 0 {
                        if let Some(base) = stack_of(host, inst.model.def.base_drop.0, inst.model.def.base_drop.1) {
                            let _ = insert_into(host, &mut e.items[2], base);
                        }
                        if success > 0
                            && let Some(pred) = models::prediction(host, inst.model, success as u32)
                        {
                            let _ = insert_into(host, &mut e.items[3], pred);
                        }
                    }
                    let mut src = stack.clone();
                    if mode == 1 && can_upgrade(host, &inst) {
                        models::set_data(&mut src, inst.data + 1);
                    }
                    let iters = models::iters_of(&src);
                    models::set_iters(&mut src, iters + 1);
                    e.nbt.insert(java("hz_model"), Tag::String(signature(&src).as_str().into()));
                    e.items[0] = Some(src);
                } else {
                    set_i(&mut e.nbt, "energy", energy - cost);
                }
            } else {
                fail = F_REDSTONE;
            }
        } else {
            fail = F_ENERGY_MID;
        }
    }
    let nbt = &mut e.nbt;
    set_i(nbt, "runtime", runtime);
    set_i(nbt, "predSuccess", success);
    set_i(nbt, "failState", fail);
    set_i(nbt, "simMode", mode);
    let energy = get_i(nbt, "energy", 0);
    e.data[0] = runtime;
    e.data[1] = success;
    e.data[2] = fail;
    e.data[3] = redstone;
    e.data[4] = mode;
    put_wide(&mut e.data, 5, energy);
}

/// clickMenuButton: 0-2 the redstone state, 3-4 the mode.
pub(crate) fn button(host: &dyn Host, nbt: &mut Compound, id: i32) {
    match id {
        0..=2 => set_i(nbt, "redstoneState", id),
        3..=4 => {
            let mode = id - 3;
            // SimModeButton is inactive while training is off; the server holds it to inference anyway.
            if mode == 1 && config::get(host).sim_model_upgrade == 0 {
                return;
            }
            set_i(nbt, "simMode", mode);
        }
        _ => {}
    }
}

/// SimItemHandler.isValid.
pub(crate) fn valid(host: &dyn Host, items: &[Option<ItemStack>], slot: usize, stack: &ItemStack) -> bool {
    match slot {
        0 => is(host, stack, "hostilenetworks:data_model"),
        1 => items.first().and_then(Option::as_ref).and_then(|m| Instance::of(host, m)).is_some_and(|i| i.model.input_matches(host, stack)),
        _ => true,
    }
}

// ---- The screen (client)

pub(crate) fn screen() -> MenuScreenLayout {
    MenuScreenLayout {
        width: 232.0,
        height: 230.0,
        slots: vec![Some(slot_at([-13.0, 1.0])), Some(slot_at([176.0, 7.0])), Some(slot_at([196.0, 7.0])), Some(slot_at([186.0, 27.0]))],
        // SimChamberMenu.addPlayerSlots(pInv, 36, 153).
        inventory_x: 36.0,
        inventory_y: 153.0,
        own_labels: true,
        own_background: true,
        ..MenuScreenLayout::default()
    }
}

/// The screen's text (containerTick): the failure message or the run's lines typing out.
#[derive(Default)]
pub(crate) struct Body {
    text: Typewriter,
    last_fail: i32,
    loaded: bool,
    last_tick: i64,
}

static BODY: Mutex<Option<Body>> = Mutex::new(None);

pub(crate) fn screen_opened() {
    *BODY.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn container_tick(host: &dyn ClientHost, body: &mut Body, data: &[i32], model: Option<&ItemStack>) {
    let (runtime, success, fail, mode) = (data[0], data[1], data[2], data[4]);
    if fail != F_NONE {
        if body.last_fail != fail {
            body.last_fail = fail;
            body.text.clear();
            let mut msg = lang(host, &fail_key(fail), "");
            if fail == F_INPUT {
                let name = model.and_then(|m| Instance::of(host, m)).map_or_else(|| "ERROR".to_owned(), |i| item_name(host, i.model.input_item()));
                msg = format_args_java(&msg, &[name]);
            }
            for (i, line) in msg.split('\n').enumerate() {
                if i == 0 {
                    body.text.add_line(line.to_owned(), 0xFFFF_FFFF, 1.0);
                } else {
                    body.text.add_line(line.to_owned(), 0xFFFF_FFFF, 1.0);
                }
            }
        }
        body.loaded = false;
    } else if !body.loaded {
        let max = runtime_of(mode);
        let ticks = (max - runtime) as f32;
        let speed = 195.0 / max as f32;
        body.text.clear();
        let iters = model.map_or(0, models::iters_of);
        let training = mode == 1;
        let prefix = if training { "hostilenetworks.run.train." } else { "hostilenetworks.run." };
        for i in 0..7 {
            let line = lang_args(host, &format!("{prefix}{i}"), "", &[iters.to_string()]);
            body.text.add_line(line, 0xFFFF_FFFF, speed);
            if i == 0 {
                body.text.continue_line(format!("v{}", crate::VERSION), argb(0xFFAA00));
            } else if i == 5 && !training {
                let ok = success > 0;
                let key = if ok { "hostilenetworks.color_text.success" } else { "hostilenetworks.color_text.failed" };
                body.text.continue_line(lang(host, key, if ok { "succeeded" } else { "failed" }), if ok { argb(0xFFAA00) } else { argb(0xFF5555) });
            }
        }
        body.text.set_ticks(ticks);
        body.loaded = true;
        body.last_fail = F_NONE;
    }
    body.text.tick();
    if runtime == 0 {
        body.loaded = false;
    }
}

pub(crate) fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, data: &[i32], o: [f32; 2]) {
    if data.len() < DATA_SLOTS {
        return;
    }
    let cfg = config::get(host);
    let tex = "hostilenetworks:gui/sim_chamber";
    let model = menu_slot(host, 0);
    let inst = model.as_ref().and_then(|m| Instance::of(host, m));
    // extractBackground.
    blit(draw, tex, o, 8.0, 0.0, 0.0, 0.0, 216.0, 141.0);
    blit(draw, tex, o, -14.0, 0.0, 0.0, 141.0, 18.0, 18.0);
    blit(draw, tex, o, 228.0, 0.0, 0.0, 141.0, 18.0, 18.0);
    blit(draw, tex, o, 228.0, 20.0, 0.0, 141.0, 18.0, 18.0);
    let energy = wide(data, 5);
    let energy_h = 87.0 - (87.0 * energy as f32 / cfg.sim_power_cap.max(1) as f32).ceil();
    blit(draw, tex, o, 211.0, 48.0, 18.0, 141.0, 7.0, energy_h);
    let data_h = match &inst {
        Some(i) if i.is_max() => 0.0,
        Some(i) => 87.0 - (87.0 * (i.data - i.tier_data()) as f32 / (i.next_tier_data() - i.tier_data()).max(1) as f32).ceil(),
        None => 87.0,
    };
    blit(draw, tex, o, 14.0, 48.0, 18.0, 141.0, 7.0, data_h);
    blit(draw, "hostilenetworks:gui/default_gui", o, 28.0, 145.0, 0.0, 0.0, 176.0, 90.0);
    // The buttons (RedstoneButton, SimModeButton).
    let redstone = data[3].clamp(0, 2) as usize;
    icon(draw, REDSTONE_TEXTURES[redstone], [o[0] + 229.0, o[1] + 1.0], None);
    let mode_tex = if data[4] == 1 { "item/experience_bottle" } else { "item/ender_eye" };
    let shade = if cfg.sim_model_upgrade != 0 { 255 } else { 89 };
    icon(draw, mode_tex, [o[0] + 229.0, o[1] + 21.0], Some(0xFF00_0000 | shade << 16 | shade << 8 | shade));
    // extractLabels.
    let runtime = data[0];
    if runtime > 0 {
        let max = runtime_of(data[4]);
        let pct = 99.min((100.0 * (max - runtime) as f32 / max as f32).ceil() as i32);
        draw.text(&format!("{pct}%"), [o[0] + 184.0, o[1] + 123.0], BLUE, true);
    }
    if let Some(i) = &inst {
        let target = lang(host, "hostilenetworks.gui.target", "Sim Target: %s");
        let (before, after) = target.split_once("%s").unwrap_or((&target, ""));
        let mut x = o[0] + 18.0;
        draw.text(before, [x, o[1] + 9.0], 0xFFFF_FFFF, true);
        x += draw.text_width(before);
        let name = models::name(host, i.model);
        draw.text(&name, [x, o[1] + 9.0], MINT, true);
        x += draw.text_width(&name);
        draw.text(after, [x, o[1] + 9.0], 0xFFFF_FFFF, true);
        let tier_line = lang(host, "hostilenetworks.gui.tier", "Model Tier: %s");
        coloured_arg(draw, &tier_line, &models::tier_name(host, i.tier), argb(TIERS[i.tier].color), [o[0] + 18.0, o[1] + 21.0]);
        let acc = format!("{}%", format_accuracy(100.0 * i.accuracy(host)));
        let acc_line = lang(host, "hostilenetworks.gui.accuracy", "Model Accuracy: %s");
        coloured_arg(draw, &acc_line, &acc, argb(TIERS[i.tier].color), [o[0] + 18.0, o[1] + 33.0]);
    }
    let mut guard = BODY.lock().unwrap_or_else(|e| e.into_inner());
    let body = guard.get_or_insert_with(|| Body { text: Typewriter::new(174.0), last_fail: F_NONE, ..Body::default() });
    let now = host.level(host.dimension()).and_then(|l| l.game_time(host)).unwrap_or(0);
    if body.last_tick != now {
        body.last_tick = now;
        container_tick(host, body, data, model.as_ref());
    }
    body.text.render(draw, [o[0] + 29.0, o[1] + 51.0]);
}

/// IAttributeExtension.FORMAT: "#.##".
pub(crate) fn format_accuracy(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_owned()
}

/// A translated line whose `%s` is drawn in its own colour.
pub(crate) fn coloured_arg(draw: &mut dyn DrawContext, pattern: &str, arg: &str, colour: u32, at: [f32; 2]) -> f32 {
    let (before, after) = pattern.split_once("%s").unwrap_or((pattern, ""));
    let mut x = at[0];
    draw.text(before, [x, at[1]], 0xFFFF_FFFF, true);
    x += draw.text_width(before);
    draw.text(arg, [x, at[1]], colour, true);
    x += draw.text_width(arg);
    draw.text(after, [x, at[1]], 0xFFFF_FFFF, true);
    x + draw.text_width(after) - at[0]
}

pub(crate) fn click(host: &dyn ClientHost, data: &[i32], at: [f32; 2]) -> Option<i32> {
    if data.len() < DATA_SLOTS {
        return None;
    }
    // RedstoneButton and SimModeButton (18 by 18 widgets at 228,0 and 228,20).
    if inside([228.0, 0.0, 18.0, 18.0], at) {
        return Some((data[3].clamp(0, 2) + 1) % 3);
    }
    if inside([228.0, 20.0, 18.0, 18.0], at) && config::get(host).sim_model_upgrade != 0 {
        return Some(3 + (1 - data[4].clamp(0, 1)));
    }
    None
}

pub(crate) fn tooltip(host: &dyn ClientHost, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if data.len() < DATA_SLOTS {
        return;
    }
    let cfg = config::get(host);
    let inst = menu_slot(host, 0).and_then(|m| Instance::of(host, &m));
    if hovering(211.0, 48.0, 7.0, 87.0, at) {
        lines.push(lang_args(host, "hostilenetworks.gui.energy", "%s/%s FE", &[wide(data, 5).to_string(), cfg.sim_power_cap.to_string()]));
        if let Some(i) = &inst {
            lines.push(lang_args(host, "hostilenetworks.gui.cost", "Model energy cost: %s FE/t", &[adjust_cost(data[4], i.model.sim_cost()).to_string()]));
        }
    } else if hovering(14.0, 48.0, 7.0, 87.0, at) {
        if let Some(i) = &inst {
            if !i.is_max() {
                let args = [(i.data - i.tier_data()).to_string(), (i.next_tier_data() - i.tier_data()).to_string()];
                lines.push(lang_args(host, "hostilenetworks.gui.data", "%s/%s Data collected", &args));
            } else {
                lines.push(format!("§c{}", lang(host, "hostilenetworks.gui.max_data", "I am at maximum potential")));
            }
        }
    } else if hovering(229.0, 1.0, 16.0, 16.0, at) {
        let key = format!("hostilenetworks.gui.redstone.{}", REDSTONE_KEYS[data[3].clamp(0, 2) as usize]);
        lines.push(lang(host, &key, &key));
    } else if hovering(229.0, 19.0, 16.0, 16.0, at) {
        if cfg.sim_model_upgrade == 0 {
            lines.push(lang(host, "hostilenetworks.gui.mode.disabled", "Training Mode is disabled by the server config"));
        } else {
            let (key, speed, power) = if data[4] == 1 { ("training", 100, 120) } else { ("inference", 125, 100) };
            let base = format!("hostilenetworks.gui.mode.{key}");
            lines.push(lang(host, &base, &base));
            lines.push(format!("§7{}", lang(host, &format!("{base}.desc"), "")));
            lines.push(format!("§7{}", lang(host, &format!("{base}.desc2"), "")));
            lines.push(String::new());
            lines.push(tip(lang_args(host, "hostilenetworks.gui.mode.power", "Energy Cost: %s%%", &[power.to_string()]), BLUE));
            lines.push(tip(lang_args(host, "hostilenetworks.gui.mode.speed", "Simulation Speed: %s%%", &[speed.to_string()]), BLUE));
        }
    }
    let _ = TIER_ORDER;
}

/// SimChamberMenu's mover rules: the machine's slots to the inventory; a data model to slot 0; the model's input to
/// slot 1; otherwise between the inventory and the hotbar. False: leave it to the game.
pub(crate) fn quick_move(host: &dyn Host, items: &mut [Option<ItemStack>], slot: usize, stack: &mut ItemStack) -> Option<std::ops::Range<usize>> {
    if slot < 4 {
        return None;
    }
    if is(host, stack, "hostilenetworks:data_model") {
        return Some(0..1);
    }
    if valid(host, items, 1, stack) {
        return Some(1..2);
    }
    None
}
