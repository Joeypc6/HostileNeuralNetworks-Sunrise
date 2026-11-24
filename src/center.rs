//! The Data Center: `DataCenterTileEntity`, `DataCenterShell`, `DataCenterMenu`, `DataCenterScreen` and
//! `DataCenterFabConfigScreen`, and its IO port (`DataCenterIOPortBlock`, `DataCenterIOPortTileEntity`).
//!
//! Slots: 0-24 models, 25-28 inputs, 29-44 outputs. Saved as the Java tile saves them (`energy`, `runtimes`,
//! `predSuccess`, `failStates`, `redstoneState`, `ejectDirIndex`, `saved_selections`), and clients get the
//! selections, `activeMask`, `shellValid` and the layout (getUpdateTag). Menu data: per model slot its runtime and
//! fail state (0-49), then the redstone state (50), the shell (51) and the energy (52-53).
//!
//! The shell keeps the chunks it spans loaded with a chunk ticket of the mod's own (Hostile.Tickets.DATA_CENTER),
//! checked again when the world loads (validateLoadedTickets).

use crate::config;
use crate::fab::{self, Ids, Selections};
use crate::gui::*;
use crate::models::{self, Instance};
use crate::models_data::TIERS;
use crate::util::*;
use mod_api::*;
use std::sync::{Mutex, OnceLock};

pub(crate) const BLOCK: &str = "hostilenetworks:data_center";
pub(crate) const PORT: &str = "hostilenetworks:data_center_io_port";
pub(crate) const DATA_SLOTS: usize = 54;
const RUNTIME: i32 = 300;

pub(crate) static TICKETS: OnceLock<TicketController> = OnceLock::new();

// ---- DataCenterShell

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Cell {
    Floor,
    Ceiling,
    Wall,
    Controller,
    Interior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    pub controller: Pos,
    /// The wall face's 3D data value (Direction.get3DDataValue).
    pub wall_face: i32,
    pub min: Pos,
}

impl Layout {
    pub fn max(&self) -> Pos {
        [self.min[0] + 6, self.min[1] + 6, self.min[2] + 6]
    }
    pub fn classify(&self, p: Pos) -> Cell {
        let max = self.max();
        if p[1] == self.min[1] {
            Cell::Floor
        } else if p[1] == max[1] {
            Cell::Ceiling
        } else if p[0] == self.min[0] || p[0] == max[0] || p[2] == self.min[2] || p[2] == max[2] {
            if p == self.controller { Cell::Controller } else { Cell::Wall }
        } else {
            Cell::Interior
        }
    }
    pub fn cells(&self) -> impl Iterator<Item = (Pos, Cell)> + '_ {
        (0..7).flat_map(move |x| (0..7).flat_map(move |y| (0..7).map(move |z| [self.min[0] + x, self.min[1] + y, self.min[2] + z])))
            .map(move |p| (p, self.classify(p)))
    }
}

/// InvalidType: obsidian_missing, glass_missing, not_air.
pub(crate) fn problem_at(host: &dyn Host, level: LevelRef, p: Pos, cell: Cell) -> Option<&'static str> {
    let block = level.block(host, p);
    match cell {
        Cell::Controller => None,
        Cell::Floor => (!block.is_some_and(|b| host.block_in_tag(b, "hostilenetworks:data_center_floor").unwrap_or(false))).then_some("obsidian_missing"),
        Cell::Ceiling | Cell::Wall => {
            (!block.is_some_and(|b| host.block_in_tag(b, "hostilenetworks:data_center_wall").unwrap_or(false))).then_some("glass_missing")
        }
        Cell::Interior => {
            let air = block.and_then(|b| b.id(host)).is_none_or(|id| matches!(id, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"));
            (!air).then_some("not_air")
        }
    }
}

pub(crate) fn invalid(host: &dyn Host, level: LevelRef, layout: &Layout, limit: usize) -> Vec<(Pos, &'static str)> {
    let mut out = Vec::new();
    for (p, cell) in layout.cells() {
        if let Some(problem) = problem_at(host, level, p, cell) {
            out.push((p, problem));
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// The candidate layouts findFor and bestGuessLayout try, from the controller's facing.
fn candidates(controller: Pos, facing: &str) -> Vec<Layout> {
    // wallFace = facing.getOpposite(); Direction 3D values: down 0, up 1, north 2, south 3, west 4, east 5.
    let (wall, axis_z) = match facing {
        "south" => ("north", true),
        "west" => ("east", false),
        "east" => ("west", false),
        _ => ("south", true),
    };
    let wall_face = match wall {
        "north" => 2,
        "south" => 3,
        "west" => 4,
        _ => 5,
    };
    let floor = controller[1] - 1;
    (0..7)
        .map(|off| {
            let min = if axis_z {
                let sz = if wall == "south" { controller[2] } else { controller[2] - 6 };
                [controller[0] - off, floor, sz]
            } else {
                let sx = if wall == "east" { controller[0] } else { controller[0] - 6 };
                [sx, floor, controller[2] - off]
            };
            Layout { controller, wall_face, min }
        })
        .collect()
}

pub(crate) fn find_for(host: &dyn Host, level: LevelRef, controller: Pos) -> Option<Layout> {
    let facing = level.block_property(host, controller, "facing")?;
    candidates(controller, &facing).into_iter().find(|l| invalid(host, level, l, 1).is_empty())
}

pub(crate) fn best_guess(host: &dyn Host, level: LevelRef, controller: Pos) -> Option<Layout> {
    let facing = level.block_property(host, controller, "facing")?;
    let mut best = None;
    let mut best_n = usize::MAX;
    for l in candidates(controller, &facing) {
        let n = invalid(host, level, &l, 50).len();
        if n < best_n {
            best_n = n;
            best = Some(l);
            if n == 0 {
                break;
            }
        }
    }
    best
}

fn needed_chunks(layout: Option<&Layout>, controller: Pos) -> Vec<[i32; 2]> {
    let Some(l) = layout else { return Vec::new() };
    let max = l.max();
    let mut out = Vec::new();
    let own = [controller[0] >> 4, controller[2] >> 4];
    for cx in (l.min[0] >> 4)..=(max[0] >> 4) {
        for cz in (l.min[2] >> 4)..=(max[2] >> 4) {
            if [cx, cz] != own {
                out.push([cx, cz]);
            }
        }
    }
    out
}

// ---- The tile

/// isSelfAware: an accuracy of at least 1.
fn self_aware(tier: usize) -> bool {
    TIERS[tier].accuracy >= 1.0
}

fn cost_of(host: &dyn Host, m: &models::Model) -> i32 {
    (m.sim_cost() as f32 * config::get(host).data_center_sim_cost).ceil() as i32
}

fn read_buffer(host: &dyn Host, nbt: &Compound) -> Vec<ItemStack> {
    match nbt.get(&java("hz_buffered")) {
        Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::Compound(c) = t { ItemStack::from_nbt(host, c) } else { None }).collect(),
        _ => Vec::new(),
    }
}

fn write_buffer(host: &dyn Host, nbt: &mut Compound, list: &[ItemStack]) {
    if list.is_empty() {
        nbt.remove(&java("hz_buffered"));
    } else {
        nbt.insert(java("hz_buffered"), Tag::List(list.iter().filter_map(|s| s.to_nbt(host)).map(Tag::Compound).collect()));
    }
}

/// insertIntoOutput: into slots 29-44 as far as they take; the rest.
fn into_output(host: &dyn Host, items: &mut [Option<ItemStack>], stack: ItemStack) -> Option<ItemStack> {
    fab::into_range(host, &mut items[29..45], stack)
}

/// canAcceptInOutput: some output slot takes at least one.
fn accepts(host: &dyn Host, items: &[Option<ItemStack>], stack: &ItemStack) -> bool {
    items[29..45].iter().any(|s| match s {
        None => true,
        Some(have) => same(have, stack) && have.count() < max_stack(host, have),
    })
}

fn has_input(host: &dyn Host, items: &[Option<ItemStack>], m: &models::Model) -> bool {
    items[25..29].iter().flatten().any(|s| m.input_matches(host, s))
}

/// serverTick.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: Pos, e: &mut Fields) {
    e.items.resize(45, None);
    if e.data.len() < DATA_SLOTS {
        e.data.resize(DATA_SLOTS, 0);
    }
    let Some(level) = host.level(host.dimension()) else { return };
    let mut runtimes = get_ints(&e.nbt, "runtimes", 25);
    let mut success = get_ints(&e.nbt, "predSuccess", 25);
    let mut fails = get_ints(&e.nbt, "failStates", 25);
    let mut valid = get_i(&e.nbt, "shellValid", 0) != 0;
    let mut timer = get_i(&e.nbt, "hz_shell_timer", 0) - 1;
    let mut sync = false;
    if timer <= 0 {
        timer = 20;
        let layout = find_for(host, level, pos);
        let was = valid;
        valid = layout.is_some();
        set_i(&mut e.nbt, "shellValid", i32::from(valid));
        match &layout {
            Some(l) => {
                set_i(&mut e.nbt, "wallFaceOrd", l.wall_face);
                set_i(&mut e.nbt, "shellMinX", l.min[0]);
                set_i(&mut e.nbt, "shellMinY", l.min[1]);
                set_i(&mut e.nbt, "shellMinZ", l.min[2]);
            }
            None => {
                for k in ["wallFaceOrd", "shellMinX", "shellMinY", "shellMinZ"] {
                    e.nbt.remove(&java(k));
                }
            }
        }
        force_chunks(host, level, pos, &mut e.nbt, layout.as_ref());
        refresh_ports(host, level, pos, &mut e.nbt, layout.as_ref());
        if valid != was {
            sync = true;
        }
    }
    set_i(&mut e.nbt, "hz_shell_timer", timer);
    let redstone = get_i(&e.nbt, "redstoneState", 0);
    let mut changed = false;
    if !valid {
        runtimes = vec![0; 25];
        fails = vec![F_SHELL; 25];
    } else if !redstone_matches(redstone, powered(host, pos)) {
        for i in 0..25 {
            if e.items[i].is_some() {
                fails[i] = F_REDSTONE;
            }
        }
    } else {
        let mut buffered = read_buffer(host, &e.nbt);
        buffered = buffered.into_iter().filter_map(|s| into_output(host, &mut e.items, s)).collect();
        let mut sels = fab::read_selections(&e.nbt);
        for i in 0..25 {
            if tick_slot(host, pos, i, e, &mut sels, &mut buffered, &mut runtimes, &mut success, &mut fails) {
                changed = true;
            }
        }
        fab::write_selections(&mut e.nbt, &sels);
        write_buffer(host, &mut e.nbt, &buffered);
    }
    // maybeSyncActiveMask.
    let mask = (0..25).filter(|i| runtimes[*i] > 0 && fails[*i] == F_NONE).fold(0i32, |m, i| m | 1 << i);
    if mask != get_i(&e.nbt, "activeMask", 0) {
        set_i(&mut e.nbt, "activeMask", mask);
        sync = true;
    }
    let _ = changed;
    set_ints(&mut e.nbt, "runtimes", &runtimes);
    set_ints(&mut e.nbt, "predSuccess", &success);
    set_ints(&mut e.nbt, "failStates", &fails);
    for i in 0..25 {
        e.data[i * 2] = runtimes[i];
        e.data[i * 2 + 1] = fails[i];
    }
    e.data[50] = redstone;
    e.data[51] = i32::from(valid);
    let energy = get_i(&e.nbt, "energy", 0);
    put_wide(&mut e.data, 52, energy);
    if sync {
        host.block_entity_sync(pos);
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_slot(
    host: &mut dyn ServerHost,
    pos: Pos,
    i: usize,
    e: &mut Fields,
    sels: &mut Selections,
    buffered: &mut Vec<ItemStack>,
    runtimes: &mut [i32],
    success: &mut [i32],
    fails: &mut [i32],
) -> bool {
    let Some(stack) = e.items[i].clone() else {
        if runtimes[i] == 0 && fails[i] == F_MODEL {
            return false;
        }
        runtimes[i] = 0;
        fails[i] = F_MODEL;
        return true;
    };
    let Some(inst) = Instance::of(host, &stack) else {
        runtimes[i] = 0;
        fails[i] = F_MODEL;
        return false;
    };
    if !TIERS[inst.tier].can_sim {
        runtimes[i] = 0;
        fails[i] = F_FAULTY;
        return false;
    }
    if !self_aware(inst.tier) {
        runtimes[i] = 0;
        fails[i] = F_NOT_SELF_AWARE;
        return false;
    }
    let m = inst.model;
    let cost = cost_of(host, m);
    let energy = get_i(&e.nbt, "energy", 0);
    if runtimes[i] == 0 {
        // canStartFor.
        let sel = sels.get(m.id()).cloned().unwrap_or_default();
        let drop = sel.current();
        if fails[i] == F_BUFFERING && !buffered.is_empty() {
            return false;
        }
        if sel.is_empty() || drop < 0 || drop as usize >= m.fab.len() {
            fails[i] = F_NO_SELECTION;
            return false;
        }
        if !has_input(host, &e.items, m) {
            fails[i] = F_INPUT;
            return false;
        }
        if energy < cost {
            fails[i] = F_ENERGY;
            return false;
        }
        let (did, dn) = m.fab[drop as usize];
        let expected = stack_of(host, did, dn);
        let base = stack_of(host, m.def.base_drop.0, m.def.base_drop.1);
        if !expected.as_ref().is_none_or(|s| accepts(host, &e.items, s)) || !base.as_ref().is_none_or(|s| accepts(host, &e.items, s)) {
            fails[i] = F_OUTPUT;
            return false;
        }
        runtimes[i] = RUNTIME;
        // consumeInput.
        if let Some(s) = (25..29).find(|s| e.items[*s].as_ref().is_some_and(|x| m.input_matches(host, x))) {
            shrink_one(&mut e.items[s]);
        }
        success[i] = inst.roll(host, [pos[0] + i as i32, pos[1], pos[2]]);
        fails[i] = F_NONE;
        true
    } else if energy < cost {
        fails[i] = F_ENERGY_MID;
        false
    } else {
        set_i(&mut e.nbt, "energy", energy - cost);
        fails[i] = F_NONE;
        runtimes[i] -= 1;
        if runtimes[i] == 0 {
            // emitOutputs.
            let sel = sels.get(m.id()).cloned().unwrap_or_default();
            if !sel.is_empty() {
                let mut over = false;
                let mut sel = sel;
                if let Some(base) = stack_of(host, m.def.base_drop.0, m.def.base_drop.1)
                    && let Some(rest) = into_output(host, &mut e.items, base)
                {
                    over = true;
                    buffered.push(rest);
                }
                for _ in 0..success[i] {
                    let idx = sel.current();
                    if idx < 0 || idx as usize >= m.fab.len() {
                        break;
                    }
                    let (id, n) = m.fab[idx as usize];
                    if let Some(drop) = stack_of(host, id, n)
                        && let Some(rest) = into_output(host, &mut e.items, drop)
                    {
                        over = true;
                        buffered.push(rest);
                    }
                    if sel.queue {
                        sel = sel.advanced();
                        sels.insert(m.id().to_owned(), sel.clone());
                    }
                }
                if over {
                    fails[i] = F_BUFFERING;
                }
            }
            let mut src = stack.clone();
            let iters = models::iters_of(&src);
            models::set_iters(&mut src, iters + 1);
            e.items[i] = Some(src);
        }
        true
    }
}

/// ensureShellChunksForced: the ticket's chunks follow the layout (the controller's own chunk excepted).
fn force_chunks(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, nbt: &mut Compound, layout: Option<&Layout>) {
    let Some(controller) = TICKETS.get().copied() else { return };
    let needed = needed_chunks(layout, pos);
    let held: Vec<[i32; 2]> = get_ints(nbt, "hz_forced", 0).chunks_exact(2).map(|c| [c[0], c[1]]).collect();
    for c in held.iter().filter(|c| !needed.contains(c)) {
        if let Some(chunk) = level.chunk_ref(host, *c) {
            chunk.set_mod_forced(host, controller, TicketOwner::Block(pos), false, false);
        }
    }
    for c in needed.iter().filter(|c| !held.contains(c)) {
        if let Some(chunk) = level.chunk_ref(host, *c) {
            chunk.set_mod_forced(host, controller, TicketOwner::Block(pos), true, false);
        }
    }
    let flat: Vec<i32> = needed.iter().flatten().copied().collect();
    set_ints(nbt, "hz_forced", &flat);
}

/// The controller is gone: its tickets go (setRemoved) and its ports forget it (clearPortOwnersOnBreak).
pub(crate) fn removed(host: &mut dyn ServerHost, pos: Pos, nbt: &Compound) {
    let Some(level) = host.level(host.dimension()) else { return };
    if let Some(controller) = TICKETS.get().copied() {
        for c in get_ints(nbt, "hz_forced", 0).chunks_exact(2) {
            if let Some(chunk) = level.chunk_ref(host, [c[0], c[1]]) {
                chunk.set_mod_forced(host, controller, TicketOwner::Block(pos), false, false);
            }
        }
    }
    for p in get_ints(nbt, "hz_ports", 0).chunks_exact(3) {
        set_owner(host, level, [p[0], p[1], p[2]], None);
    }
}

/// refreshOwnedPorts: the shell's ports are told their owner; ports no longer in it are told they have none.
fn refresh_ports(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, nbt: &mut Compound, layout: Option<&Layout>) {
    let mut current = Vec::new();
    if let Some(l) = layout {
        for (p, cell) in l.cells() {
            if matches!(cell, Cell::Wall | Cell::Ceiling) && level.block(host, p).is_some_and(|b| b.id(host) == Some(PORT)) {
                current.push(p);
                set_owner(host, level, p, Some(pos));
            }
        }
    }
    let before: Vec<Pos> = get_ints(nbt, "hz_ports", 0).chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    for p in before.iter().filter(|p| !current.contains(p)) {
        set_owner(host, level, *p, None);
    }
    let flat: Vec<i32> = current.iter().flatten().copied().collect();
    set_ints(nbt, "hz_ports", &flat);
}

/// BlockPos.asLong.
pub(crate) fn pack(p: Pos) -> i64 {
    ((p[0] as i64 & 0x3FF_FFFF) << 38) | ((p[2] as i64 & 0x3FF_FFFF) << 12) | (p[1] as i64 & 0xFFF)
}

pub(crate) fn unpack(v: i64) -> Pos {
    [(v >> 38) as i32, ((v << 52) >> 52) as i32, ((v << 26) >> 38) as i32]
}

/// DataCenterIOPortTileEntity.setOwner / clearOwner: saved as `owner` (a packed position).
fn set_owner(host: &mut dyn ServerHost, level: LevelRef, port: Pos, owner: Option<Pos>) {
    let Some(be) = host.block_entity(level, port) else { return };
    let now = be.data(&*host).and_then(|d| match d.get(&java("owner")) {
        Some(Tag::Long(v)) => Some(*v),
        _ => None,
    });
    let want = owner.map(pack);
    if now == want {
        return;
    }
    let mut data = be.data(&*host).unwrap_or_default();
    match want {
        Some(v) => {
            data.insert(java("owner"), Tag::Long(v));
        }
        None => {
            data.remove(&java("owner"));
        }
    }
    let _ = be.replace_data(host, data);
}

/// DataCenterIOPortTileEntity.resolveOwner: the controller whose shell holds the port, if its shell is whole.
pub(crate) fn port_owner(host: &dyn Host, level: LevelRef, port: Pos) -> Option<Pos> {
    let data = host.block_entity(level, port)?.data(host)?;
    let Some(Tag::Long(v)) = data.get(&java("owner")) else { return None };
    let owner = unpack(*v);
    let be = host.block_entity(level, owner)?;
    if level.block(host, owner).and_then(|b| b.id(host)) != Some(BLOCK) {
        return None;
    }
    let shell = be.data(host).is_some_and(|d| get_i(&d, "shellValid", 0) != 0);
    shell.then_some(owner)
}

/// `port_owner` as a capability provider reads it, through the level's read-only view: the controller the port at
/// `port` names, while its shell is whole.
pub(crate) fn port_owner_in(level: &dyn LevelHost, port: BlockEntityRef) -> Option<Pos> {
    let data = level.block_entity_data(port)?;
    let Some(Tag::Long(v)) = data.get(&java("owner")) else { return None };
    let owner = unpack(*v);
    let be = level.block_entity(owner)?;
    if level.block_state(owner).and_then(|s| level.state_block(s)) != Some(BLOCK) {
        return None;
    }
    let shell = level.block_entity_data(be).is_some_and(|d| get_i(&d, "shellValid", 0) != 0);
    shell.then_some(owner)
}

/// DataCenterIOPortBlock.useWithoutItem: the mode cycles, with a message.
pub(crate) fn cycle_port(host: &mut dyn ServerHost, click: &BlockClick<'_>) -> Option<InteractionResult> {
    let level = host.level(host.dimension())?;
    let now = level.block_property(&*host, click.pos, "mode").unwrap_or_else(|| "energy".to_owned());
    let next = match now.as_str() {
        "energy" => "models",
        "models" => "inputs",
        "inputs" => "outputs",
        _ => "energy",
    };
    host.set_block_property_with(click.pos, "mode", next, 3);
    if let Some(p) = host.player(click.player) {
        let line = Text::translate("hostilenetworks.io_port.mode_set", vec![Text::translate(format!("hostilenetworks.io_port.mode.{next}"), Vec::new())]);
        let _ = p.action_bar(host, line);
    }
    Some(InteractionResult::Success)
}

/// DataCenterItemHandler.isValid.
pub(crate) fn valid(host: &dyn Host, items: &[Option<ItemStack>], slot: usize, stack: &ItemStack) -> bool {
    if slot < 25 {
        is(host, stack, "hostilenetworks:data_model")
    } else if slot < 29 {
        items.iter().take(25).flatten().filter_map(|m| Instance::of(host, m)).any(|i| i.model.input_matches(host, stack))
    } else {
        true
    }
}

/// DataCenterMenu.clickMenuButton.
pub(crate) fn button(host: &dyn Host, e: &mut Fields, id: i32) -> bool {
    if (2000..2003).contains(&id) {
        set_i(&mut e.nbt, "redstoneState", id - 2000);
        return false;
    }
    let simple = |base: i32| (base..base + 25).contains(&id).then(|| (id - base) as usize);
    let composite = |base: i32| (base..base + 2500).contains(&id).then(|| ((id - base) / 100) as usize);
    let model_at = |slot: usize| e.items.get(slot).and_then(Option::as_ref).and_then(|s| models::stored(host, s));
    let mut sels = fab::read_selections(&e.nbt);
    if let Some(slot) = simple(4000) {
        if let Some(m) = model_at(slot) {
            fab::cycle_mode(&mut sels, m.id());
        }
    } else if let Some(slot) = simple(4100) {
        if let Some(m) = model_at(slot) {
            fab::set_fixed(&mut sels, m, -1);
        }
    } else if let Some(slot) = simple(4200) {
        if let Some(m) = model_at(slot) {
            fab::clear_queue(&mut sels, m.id());
        }
    } else if let Some(slot) = composite(5000) {
        if let Some(m) = model_at(slot) {
            let drop = (id - 5000) % 100;
            if sels.get(m.id()).is_some_and(|s| s.queue) {
                fab::append(&mut sels, m, drop);
            } else {
                fab::set_fixed(&mut sels, m, drop);
            }
        }
    } else if let Some(slot) = composite(8000) {
        if let Some(m) = model_at(slot) {
            fab::remove_from_queue(&mut sels, m.id(), (id - 8000) % 100);
        }
    } else {
        return false;
    }
    fab::write_selections(&mut e.nbt, &sels);
    true
}

// ---- The screens (client)

pub(crate) fn screen() -> MenuScreenLayout {
    let mut slots = Vec::new();
    for row in 0..5 {
        for col in 0..5 {
            slots.push(Some(slot_at([9.0 + col as f32 * 18.0, 21.0 + row as f32 * 20.0])));
        }
    }
    for col in 0..4 {
        slots.push(Some(slot_at([132.0 + col as f32 * 18.0, 20.0])));
    }
    for row in 0..4 {
        for col in 0..4 {
            slots.push(Some(slot_at([132.0 + col as f32 * 18.0, 48.0 + row as f32 * 18.0])));
        }
    }
    MenuScreenLayout { width: 230.0, height: 223.0, slots, inventory_x: 35.0, inventory_y: 140.0, own_labels: true, own_background: true, ..MenuScreenLayout::default() }
}

/// The model slot whose configuration screen is open over the Data Center's, on this client.
pub(crate) static CONFIG: Mutex<Option<usize>> = Mutex::new(None);

pub(crate) fn config_slot() -> Option<usize> {
    *CONFIG.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn close_config() {
    *CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn viewed_nbt(host: &dyn ClientHost) -> Option<Compound> {
    let pos = host.screen_pos()?;
    let level = host.level(host.dimension())?;
    host.block_entity(level, pos)?.data(host)
}

pub(crate) fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, data: &[i32], o: [f32; 2]) {
    if data.len() < DATA_SLOTS {
        return;
    }
    let cfg = config::get(host);
    let tex = "hostilenetworks:gui/data_center";
    blit(draw, tex, o, 0.0, 0.0, 0.0, 0.0, 230.0, 128.0);
    let cap = cfg.data_center_power_cap.max(1);
    let bar = ((100.0 * wide(data, 52) as f32 / cap as f32).floor()).clamp(0.0, 100.0);
    blit(draw, tex, o, 213.0, 19.0 + 100.0 - bar, 231.0, 0.0, 7.0, bar);
    for row in 0..5 {
        for col in 0..5 {
            let idx = row * 5 + col;
            let fs = data[idx * 2 + 1];
            if fs == F_MODEL {
                continue;
            }
            let rt = data[idx * 2];
            let (x, y) = (9.0 + col as f32 * 18.0, 21.0 + row as f32 * 20.0 + 16.0);
            let (filled, v) = if fs != F_NONE {
                (16.0, 131.0)
            } else {
                if rt <= 0 {
                    continue;
                }
                ((16.0 - (16.0 * rt as f32 / RUNTIME as f32).floor()).clamp(0.0, 16.0), 129.0)
            };
            blit(draw, tex, o, x, y, 0.0, v, filled, 2.0);
        }
    }
    blit(draw, tex, o, -22.0, 0.0, 18.0, 138.0, 18.0, 18.0);
    icon(draw, REDSTONE_TEXTURES[data[50].clamp(0, 2) as usize], [o[0] - 21.0, o[1] + 1.0], None);
    blit(draw, "hostilenetworks:gui/default_gui", o, 27.0, 132.0, 0.0, 0.0, 176.0, 90.0);
    // extractLabels.
    draw.text(&block_name(host, BLOCK), [o[0] + 8.0, o[1] + 6.0], BLUE, false);
    draw.text(&lang(host, "hostilenetworks.gui.data_center.io", "Input / Output"), [o[0] + 130.0, o[1] + 6.0], BLUE, false);
}

pub(crate) fn click(data: &[i32], at: [f32; 2]) -> Option<i32> {
    if data.len() < DATA_SLOTS {
        return None;
    }
    hovering(-22.0, 0.0, 18.0, 18.0, at).then(|| 2000 + (data[50].clamp(0, 2) + 1) % 3)
}

/// A right press on a model slot opens its configuration screen (mouseClicked, button 1); true when it did.
pub(crate) fn right_press(host: &dyn ClientHost, at: [f32; 2]) -> bool {
    for row in 0..5 {
        for col in 0..5 {
            if hovering(9.0 + col as f32 * 18.0, 21.0 + row as f32 * 20.0, 16.0, 16.0, at) {
                let idx = row * 5 + col;
                if menu_slot(host, idx).is_some_and(|s| models::stored(host, &s).is_some()) {
                    *CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(idx);
                    *fab::PAGE.lock().unwrap_or_else(|e| e.into_inner()) = 0;
                    return true;
                }
            }
        }
    }
    false
}

/// The screen's tooltips: (lines, drawn on the left of the screen).
pub(crate) fn tooltip(host: &dyn ClientHost, data: &[i32], at: [f32; 2]) -> (Vec<String>, Vec<String>) {
    let mut lines = Vec::new();
    let mut left = Vec::new();
    if data.len() < DATA_SLOTS {
        return (lines, left);
    }
    let cfg = config::get(host);
    if hovering(213.0, 19.0, 7.0, 100.0, at) {
        lines.push(lang_args(host, "hostilenetworks.gui.energy", "%s/%s FE", &[wide(data, 52).to_string(), cfg.data_center_power_cap.to_string()]));
    }
    if hovering(-22.0, 0.0, 18.0, 18.0, at) {
        let key = format!("hostilenetworks.gui.redstone.{}", REDSTONE_KEYS[data[50].clamp(0, 2) as usize]);
        lines.push(lang(host, &key, &key));
    }
    if data[51] == 0 {
        left.push(tip(lang(host, "hostilenetworks.fail.shell_broken", "Multiblock shell is incomplete or invalid"), RED_TEXT));
        if let (Some(pos), Some(level)) = (host.screen_pos(), host.level(host.dimension()))
            && let Some(layout) = best_guess(host, level, pos)
        {
            let bad = invalid(host, level, &layout, 50);
            if bad.len() == 1 {
                let (p, kind) = bad[0];
                let kind = lang(host, &format!("hostilenetworks.gui.data_center.shell_invalid.{kind}"), kind);
                let args = [kind, p[0].to_string(), p[1].to_string(), p[2].to_string()];
                left.push(tip(lang_args(host, "hostilenetworks.gui.data_center.shell_invalid.at_pos", "%s at (%s, %s, %s)", &args), ORANGE_TEXT));
            } else if bad.len() > 1 {
                let ys: std::collections::BTreeSet<i32> = bad.iter().map(|(p, _)| p[1]).collect();
                let list = ys.iter().map(i32::to_string).collect::<Vec<_>>().join(", ");
                left.push(tip(lang_args(host, "hostilenetworks.gui.data_center.shell_invalid.multi_y", "Issues at Y = %s", &[list]), ORANGE_TEXT));
            }
        }
        return (lines, left);
    }
    let nbt = viewed_nbt(host).unwrap_or_default();
    for row in 0..5 {
        for col in 0..5 {
            if !hovering(9.0 + col as f32 * 18.0, 21.0 + row as f32 * 20.0, 16.0, 16.0, at) {
                continue;
            }
            let idx = row * 5 + col;
            let Some(stack) = menu_slot(host, idx) else { continue };
            let fs = data[idx * 2 + 1];
            let model = models::stored(host, &stack);
            left.push(model.map_or_else(|| item_name(host, "hostilenetworks:data_model"), |m| model_item_name(host, "data_model", m)));
            if fs != F_NONE {
                let mut msg = lang(host, &fail_key(fs), "");
                if fs == F_INPUT {
                    let name = model.map_or_else(|| "?".to_owned(), |m| item_name(host, m.input_item()));
                    msg = format_args_java(&msg, &[name]);
                }
                for l in msg.split('\n') {
                    left.push(tip(l, RED_TEXT));
                }
            } else if let Some(m) = model {
                let cost = (m.sim_cost() as f32 * cfg.data_center_sim_cost).ceil() as i32;
                left.push(tip(lang_args(host, "hostilenetworks.gui.data_center.power_cost", "Power: %s FE/t", &[cost.to_string()]), BLUE));
                let sel = fab::selection(&nbt, m.id());
                let drop = sel.current();
                if drop >= 0
                    && let Some((id, n)) = m.fab.get(drop as usize)
                {
                    let label = if *n == 1 { item_name(host, id) } else { format!("{n} {}", item_name(host, id)) };
                    left.push(tip(lang_args(host, "hostilenetworks.gui.data_center.producing", "Producing: %s", &[label]), BLUE));
                }
                let runtime = data[idx * 2];
                if runtime > 0 {
                    let elapsed = format!("{:.1}", (RUNTIME - runtime) as f32 / 20.0);
                    left.push(tip(lang_args(host, "hostilenetworks.gui.data_center.progress", "Progress: %ss / %ss", &[elapsed, "15.0".to_owned()]), BLUE));
                }
            }
            left.push(tip(lang(host, "hostilenetworks.gui.data_center.click_to_config", "Right-click to configure"), GRAY_TEXT));
            return (lines, left);
        }
    }
    (lines, left)
}

/// "%s Data Model" / "%s Prediction" with the model's name (DataModelItem.getName, MobPredictionItem.getName).
pub(crate) fn model_item_name(host: &dyn ClientHost, item: &str, m: &models::Model) -> String {
    let key = format!("item.hostilenetworks.{item}");
    lang_args(host, &key, "%s", &[models::name(host, m)])
}

// ---- DataCenterFabConfigScreen: drawn over the Data Center's screen, 176 by 100 in the window's middle

/// Where the configuration screen was drawn last, for its clicks.
pub(crate) static CONFIG_ORIGIN: Mutex<[f32; 2]> = Mutex::new([0.0, 0.0]);

pub(crate) fn config_origin(size: [f32; 2]) -> [f32; 2] {
    [((size[0] - 176.0) / 2.0).floor(), ((size[1] - 100.0) / 2.0).floor()]
}

pub(crate) fn draw_config(host: &dyn ClientHost, draw: &mut dyn DrawContext, slot: usize, mouse: [f32; 2]) {
    let [w, h] = draw.size();
    // extractMenuBackground: the screen behind is dimmed.
    draw.fill([0.0, 0.0, w, h], 0xC010_1010);
    draw.fill([0.0, 0.0, w, h], 0xC010_1010);
    let o = config_origin([w, h]);
    *CONFIG_ORIGIN.lock().unwrap_or_else(|e| e.into_inner()) = o;
    blit(draw, "hostilenetworks:gui/loot_fabricator", o, 0.0, 0.0, 0.0, 0.0, 176.0, 83.0);
    blit(draw, "hostilenetworks:gui/data_center", o, -22.0, 0.0, 36.0, 138.0, 18.0, 18.0);
    let Some(m) = menu_slot(host, slot).and_then(|s| models::stored(host, &s)) else { return };
    let nbt = viewed_nbt(host).unwrap_or_default();
    let sel = fab::selection(&nbt, m.id());
    fab::draw_picker(host, draw, o, m, &sel, mouse);
    draw.text(&models::name(host, m), [o[0] + 8.0, o[1] - 10.0], BLUE, false);
    // renderHoverTooltip.
    let at = [mouse[0] - o[0], mouse[1] - o[1]];
    let mut tip_lines = Vec::new();
    if hovering(-22.0, 0.0, 18.0, 18.0, at) {
        tip_lines.push(lang(host, "hostilenetworks.gui.data_center.back", "Back to Data Center"));
    }
    let (more, _) = fab::picker_tooltip(host, m, &sel, at);
    tip_lines.extend(more);
    if !tip_lines.is_empty() {
        let texts: Vec<Text> = tip_lines.iter().map(|l| legacy_text(l)).collect();
        let _ = draw.gui_tooltip(&texts, mouse);
    }
}

/// A press on the configuration screen: the menu buttons it sends, and whether it closes.
pub(crate) fn config_click(host: &dyn ClientHost, slot: usize, mouse: [f32; 2]) -> (Vec<i32>, bool) {
    let o = *CONFIG_ORIGIN.lock().unwrap_or_else(|e| e.into_inner());
    let at = [mouse[0] - o[0], mouse[1] - o[1]];
    if hovering(-22.0, 0.0, 18.0, 18.0, at) {
        return (Vec::new(), true);
    }
    let Some(m) = menu_slot(host, slot).and_then(|s| models::stored(host, &s)) else { return (Vec::new(), false) };
    if fab::page_click(m, at) {
        return (Vec::new(), false);
    }
    let nbt = viewed_nbt(host).unwrap_or_default();
    let sel = fab::selection(&nbt, m.id());
    let base = slot as i32;
    let ids = Ids { cycle: 4000 + base, clear_fixed: 4100 + base, clear_queue: 4200 + base, drop: 5000 + base * 100, remove: 8000 + base * 100 };
    (fab::picker_clicks(m, &sel, at, &ids), false)
}
