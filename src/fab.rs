//! The Loot Fabricator: `LootFabTileEntity`, `LootFabMenu`, `LootFabScreen`, `FabSelection` and the drop picker the
//! Data Center's configuration screen shares (`DataCenterFabConfigScreen`).
//!
//! Slots: 0 the prediction, 1-16 the outputs. Saved as the Java tile saves them (`saved_selections`, `energy`,
//! `runtime`, `selection`, `redstoneState`); clients get `saved_selections` (getUpdateTag). Menu data: 0 runtime,
//! 1 redstone state, 2-3 energy.

use crate::config;
use crate::gui::*;
use crate::models::{self, Model};
use crate::util::*;
use mod_api::*;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub(crate) const BLOCK: &str = "hostilenetworks:loot_fabricator";
pub(crate) const DATA_SLOTS: usize = 4;

// ---- FabSelection

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    pub queue: bool,
    pub entries: Vec<i32>,
    pub cursor: i32,
}

impl Selection {
    pub fn fixed(index: i32) -> Self {
        Self { queue: false, entries: vec![index], cursor: 0 }
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// current: the entry at the cursor, -1 for none.
    pub fn current(&self) -> i32 {
        if self.entries.is_empty() { -1 } else { self.entries[self.cursor.rem_euclid(self.entries.len() as i32) as usize] }
    }
    pub fn advanced(&self) -> Self {
        if self.queue && !self.entries.is_empty() {
            Self { cursor: (self.cursor + 1) % self.entries.len() as i32, ..self.clone() }
        } else {
            self.clone()
        }
    }
    pub fn to_tag(&self) -> Tag {
        let mut c = Compound::new();
        c.insert(java("mode"), Tag::String(if self.queue { "queue" } else { "fixed" }.into()));
        c.insert(java("entries"), Tag::IntArray(self.entries.clone().into()));
        c.insert(java("cursor"), Tag::Int(self.cursor));
        Tag::Compound(c)
    }
    /// FabSelection.CODEC, or a bare int (the old format and SavedSelections' VALUE_CODEC).
    pub fn from_tag(tag: &Tag) -> Self {
        match tag {
            Tag::Int(i) => Self::fixed(*i),
            Tag::Compound(c) => Self {
                queue: get_s(c, "mode").as_deref() == Some("queue"),
                entries: match c.get(&java("entries")) {
                    Some(Tag::IntArray(a)) => a.to_vec(),
                    Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::Int(v) = t { Some(*v) } else { None }).collect(),
                    _ => Vec::new(),
                },
                cursor: get_i(c, "cursor", 0),
            },
            _ => Self::default(),
        }
    }
}

pub(crate) type Selections = BTreeMap<String, Selection>;

pub(crate) fn read_selections(nbt: &Compound) -> Selections {
    let mut out = Selections::new();
    if let Some(c) = get_c(nbt, "saved_selections") {
        for (k, v) in c {
            out.insert(k.to_string_lossy(), Selection::from_tag(v));
        }
    }
    out
}

pub(crate) fn write_selections(nbt: &mut Compound, sels: &Selections) {
    let mut c = Compound::new();
    for (k, v) in sels {
        c.insert(java(k), v.to_tag());
    }
    nbt.insert(java("saved_selections"), Tag::Compound(c));
}

pub(crate) fn selection(nbt: &Compound, model: &str) -> Selection {
    read_selections(nbt).remove(model).unwrap_or_default()
}

/// The tile's selection operations (cycleMode, setFixedDrop, appendToQueue, clearQueue, removeFromQueue), on a
/// model's entry of `sels`.
pub(crate) fn cycle_mode(sels: &mut Selections, model: &str) {
    let sel = sels.get(model).cloned().unwrap_or_default();
    let updated = if !sel.queue {
        Selection { queue: true, entries: sel.entries.clone(), cursor: 0 }
    } else {
        let current = sel.current();
        if current == -1 { Selection::default() } else { Selection::fixed(current) }
    };
    sels.insert(model.to_owned(), updated);
}

pub(crate) fn set_fixed(sels: &mut Selections, m: &Model, index: i32) {
    if index == -1 {
        sels.remove(m.id());
    } else {
        let last = (m.fab.len() as i32 - 1).max(0);
        sels.insert(m.id().to_owned(), Selection::fixed(index.clamp(0, last)));
    }
}

pub(crate) fn append(sels: &mut Selections, m: &Model, index: i32) {
    if index < 0 || index as usize >= m.fab.len() {
        return;
    }
    let sel = sels.get(m.id()).cloned().unwrap_or_default();
    let mut entries = sel.entries.clone();
    entries.push(index);
    sels.insert(m.id().to_owned(), Selection { queue: true, entries, cursor: sel.cursor });
}

pub(crate) fn clear_queue(sels: &mut Selections, model: &str) {
    if sels.get(model).is_some_and(|s| s.queue) {
        sels.insert(model.to_owned(), Selection { queue: true, entries: Vec::new(), cursor: 0 });
    }
}

pub(crate) fn remove_from_queue(sels: &mut Selections, model: &str, pos: i32) {
    let Some(sel) = sels.get(model).cloned() else { return };
    if pos < 0 || pos as usize >= sel.entries.len() {
        return;
    }
    let mut entries = sel.entries.clone();
    entries.remove(pos as usize);
    let mut cursor = sel.cursor;
    if pos < cursor {
        cursor -= 1;
    }
    cursor = if entries.is_empty() { 0 } else { cursor.rem_euclid(entries.len() as i32) };
    sels.insert(model.to_owned(), Selection { queue: true, entries, cursor });
}

/// getSelectedDrop: the selection's current drop if it is one of the model's.
pub(crate) fn selected_drop(sel: &Selection, m: &Model) -> i32 {
    let index = sel.current();
    if index >= 0 && (index as usize) < m.fab.len() { index } else { -1 }
}

// ---- The tile

/// serverTick.
pub(crate) fn tick(host: &mut dyn ServerHost, pos: Pos, e: &mut Fields) {
    let cfg = config::get(host);
    e.items.resize(17, None);
    if e.data.len() < DATA_SLOTS {
        e.data.resize(DATA_SLOTS, 0);
    }
    let mut runtime = get_i(&e.nbt, "runtime", 0);
    let redstone = get_i(&e.nbt, "redstoneState", 0);
    let model = e.items[0].as_ref().and_then(|s| models::stored(host, s));
    'run: {
        let Some(m) = model else {
            runtime = 0;
            break 'run;
        };
        let mut sels = read_selections(&e.nbt);
        let sel = sels.get(m.id()).cloned().unwrap_or_default();
        let chosen = selected_drop(&sel, m);
        if get_i(&e.nbt, "selection", -1) != chosen {
            set_i(&mut e.nbt, "selection", chosen);
            runtime = 0;
            break 'run;
        }
        if chosen == -1 {
            runtime = 0;
            break 'run;
        }
        if !redstone_matches(redstone, powered(host, pos)) {
            break 'run;
        }
        if runtime >= 60 {
            let (id, n) = m.fab[chosen as usize];
            let Some(out) = stack_of(host, id, n) else { break 'run };
            // tryInsertInOutput(out, true), then for real.
            let mut copy: Vec<Option<ItemStack>> = e.items[1..17].to_vec();
            if into_range(host, &mut copy, out).is_none() {
                runtime = 0;
                for (i, s) in copy.into_iter().enumerate() {
                    e.items[1 + i] = s;
                }
                shrink_one(&mut e.items[0]);
                if sel.queue {
                    sels.insert(m.id().to_owned(), sel.advanced());
                    write_selections(&mut e.nbt, &sels);
                    host.block_entity_sync(pos);
                }
            }
        } else {
            let energy = get_i(&e.nbt, "energy", 0);
            if energy < cfg.fab_power_cost {
                break 'run;
            }
            set_i(&mut e.nbt, "energy", energy - cfg.fab_power_cost);
            runtime += 1;
        }
    }
    set_i(&mut e.nbt, "runtime", runtime);
    let energy = get_i(&e.nbt, "energy", 0);
    e.data[0] = runtime;
    e.data[1] = redstone;
    put_wide(&mut e.data, 2, energy);
}

/// Into the slots, each as far as it takes (insertInternal in order); what didn't fit.
pub(crate) fn into_range(host: &dyn Host, slots: &mut [Option<ItemStack>], stack: ItemStack) -> Option<ItemStack> {
    let mut left = Some(stack);
    for slot in slots.iter_mut() {
        let Some(s) = left.take() else { return None };
        left = insert_into(host, slot, s);
    }
    left
}

/// LootFabMenu.clickMenuButton.
pub(crate) fn button(host: &dyn Host, pos: Pos, e: &mut Fields, id: i32) -> bool {
    if (2000..2003).contains(&id) {
        set_i(&mut e.nbt, "redstoneState", id - 2000);
        return false;
    }
    let Some(m) = e.items.first().and_then(Option::as_ref).and_then(|s| models::stored(host, s)) else { return false };
    let mut sels = read_selections(&e.nbt);
    match id {
        -3 => clear_queue(&mut sels, m.id()),
        -2 => {
            cycle_mode(&mut sels, m.id());
            set_i(&mut e.nbt, "runtime", 0);
        }
        -1 => set_fixed(&mut sels, m, -1),
        1000..=1999 => remove_from_queue(&mut sels, m.id(), id - 1000),
        0..=999 if (id as usize) < m.fab.len() => {
            if sels.get(m.id()).is_some_and(|s| s.queue) {
                append(&mut sels, m, id);
            } else {
                set_fixed(&mut sels, m, id);
            }
        }
        _ => return false,
    }
    write_selections(&mut e.nbt, &sels);
    let _ = pos;
    true
}

// ---- The screen (client)

pub(crate) fn screen() -> MenuScreenLayout {
    let mut slots = vec![Some(slot_at([79.0, 62.0]))];
    for y in 0..4 {
        for x in 0..4 {
            slots.push(Some(slot_at([100.0 + x as f32 * 18.0, 7.0 + y as f32 * 18.0])));
        }
    }
    MenuScreenLayout { width: 176.0, height: 178.0, slots, inventory_x: 8.0, inventory_y: 96.0, own_labels: true, own_background: true, ..MenuScreenLayout::default() }
}

/// The drop picker's page, on this client (LootFabScreen.currentPage and the configuration screen's).
pub(crate) static PAGE: Mutex<usize> = Mutex::new(0);

pub(crate) fn page() -> usize {
    *PAGE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The fabricator's selection for the model in its slot, from the client's copy of the block entity.
fn viewed_selection(host: &dyn ClientHost, m: &Model) -> Selection {
    let Some(pos) = host.screen_pos() else { return Selection::default() };
    let Some(level) = host.level(host.dimension()) else { return Selection::default() };
    let Some(be) = host.block_entity(level, pos) else { return Selection::default() };
    be.data(host).map(|nbt| selection(&nbt, m.id())).unwrap_or_default()
}

/// The picker the Loot Fabricator screen and the Data Center's configuration screen draw: its drops, the chosen one,
/// the mode button and, in queue mode, the queue. `o` is the panel's corner.
pub(crate) fn draw_picker(host: &dyn ClientHost, draw: &mut dyn DrawContext, o: [f32; 2], m: &Model, sel: &Selection, mouse: [f32; 2]) {
    let tex = "hostilenetworks:gui/loot_fabricator";
    let page = page();
    let at = [mouse[0] - o[0], mouse[1] - o[1]];
    let queue = sel.queue;
    let drops = drop_stacks(host, m);
    let on_page = (drops.len() as i64 - page as i64 * 9).clamp(0, 9) as usize;
    for y in 0..3 {
        for x in 0..3 {
            if y * 3 + x < on_page && hovering(18.0 + 18.0 * x as f32, 10.0 + 18.0 * y as f32, 16.0, 16.0, at) {
                blit(draw, tex, o, 16.0 + 19.0 * x as f32, 8.0 + 19.0 * y as f32, 13.0, 83.0, 18.0, 18.0);
            }
        }
    }
    let chosen = selected_drop(sel, m);
    if !queue && chosen != -1 && chosen as usize / 9 == page {
        let i = chosen as usize - page * 9;
        blit(draw, tex, o, 16.0 + 19.0 * (i % 3) as f32, 8.0 + 19.0 * (i / 3) as f32, 31.0, 83.0, 18.0, 18.0);
    }
    if chosen != -1
        && let Some(Some(s)) = drops.get(chosen as usize)
    {
        draw_stack(draw, s, [o[0] + 79.0, o[1] + 5.0]);
    }
    if queue {
        draw_queue(draw, o, sel, &drops);
    }
    for i in 0..on_page {
        if let Some(Some(s)) = drops.get(i + page * 9) {
            draw_stack(draw, s, [o[0] + 17.0 + (i % 3) as f32 * 19.0, o[1] + 9.0 + (i / 3) as f32 * 19.0]);
        }
    }
    blit(draw, tex, o, 180.0, 0.0, 31.0, 83.0, 18.0, 18.0);
    icon(draw, if queue { "item/knowledge_book" } else { "item/item_frame" }, [o[0] + 181.0, o[1] + 1.0], None);
    if queue {
        blit(draw, tex, o, 200.0, 0.0, 31.0, 83.0, 18.0, 18.0);
        icon(draw, "item/barrier", [o[0] + 201.0, o[1] + 1.0], None);
        draw.text(&lang(host, "hostilenetworks.gui.queue_current", "Current Queue"), [o[0] + 180.0, o[1] + 20.0], BLUE, true);
    }
    // The page buttons (ImageButtons at 13,68 and 46,68, 29 by 12).
    let pages = m.fab.len() / 9;
    for (x, show, name) in [(13.0, page > 0, "fab_left"), (46.0, page < pages, "fab_right")] {
        if show {
            let hover = inside([x, 68.0, 29.0, 12.0], at);
            let sprite = format!("hostilenetworks:gui/sprites/widget/{name}{}", if hover { "_hovered" } else { "" });
            draw.sprite(&sprite, [0.0, 0.0, 29.0, 12.0], [o[0] + x, o[1] + 68.0, 29.0, 12.0], None);
        }
    }
}

/// An item and its count, as a slot draws it.
pub(crate) fn draw_stack(draw: &mut dyn DrawContext, s: &ItemStack, at: [f32; 2]) {
    draw.item_stack(s, at, 16.0);
    if s.count() > 1 {
        let n = s.count().to_string();
        let w = draw.text_width(&n);
        draw.text(&n, [at[0] + 17.0 - w, at[1] + 9.0], 0xFFFF_FFFF, true);
    }
}

fn draw_queue(draw: &mut dyn DrawContext, o: [f32; 2], sel: &Selection, drops: &[Option<ItemStack>]) {
    let tex = "hostilenetworks:gui/loot_fabricator";
    let size = sel.entries.len();
    let overflow = size > 9;
    let visible = if overflow { 8 } else { size };
    let cursor = if size == 0 { -1 } else { sel.cursor.rem_euclid(size as i32) };
    for i in 0..9 {
        let (cx, cy) = (180.0 + (i % 3) as f32 * 18.0, 30.0 + (i / 3) as f32 * 18.0);
        let is_cursor = i < visible && i as i32 == cursor;
        blit(draw, tex, o, cx, cy, if is_cursor { 31.0 } else { 13.0 }, 83.0, 18.0, 18.0);
        if i < visible {
            let idx = sel.entries[i];
            if idx >= 0
                && let Some(Some(s)) = drops.get(idx as usize)
            {
                draw_stack(draw, s, [o[0] + cx + 1.0, o[1] + cy + 1.0]);
            }
        } else if i == 8 && overflow {
            let txt = format!("+{}", size - visible);
            let w = draw.text_width(&txt);
            draw.text(&txt, [o[0] + cx + 1.0 + ((16.0 - w) / 2.0).floor(), o[1] + cy + 2.0 + 3.0], 0xFFFF_FFFF, true);
        }
    }
}

pub(crate) fn drop_stacks(host: &dyn Host, m: &Model) -> Vec<Option<ItemStack>> {
    m.fab.iter().map(|(id, n)| stack_of(host, id, *n)).collect()
}

/// The picker's buttons under the pointer: the ids LootFabScreen.mouseClicked sends, with `base` the menu's id bases
/// (the fabricator's, or the Data Center's for a slot). None when the press is on none.
pub(crate) struct Ids {
    pub cycle: i32,
    pub clear_fixed: i32,
    pub clear_queue: i32,
    pub drop: i32,
    pub remove: i32,
}

pub(crate) fn picker_clicks(m: &Model, sel: &Selection, at: [f32; 2], ids: &Ids) -> Vec<i32> {
    let mut out = Vec::new();
    let page = page();
    let queue = sel.queue;
    let chosen = selected_drop(sel, m);
    if hovering(180.0, 0.0, 18.0, 18.0, at) {
        out.push(ids.cycle);
    }
    for y in 0..3 {
        for x in 0..3 {
            let idx = page * 9 + y * 3 + x;
            if idx < m.fab.len() && hovering(18.0 + 18.0 * x as f32, 10.0 + 18.0 * y as f32, 16.0, 16.0, at) && (queue || chosen != idx as i32) {
                out.push(ids.drop + idx as i32);
            }
        }
    }
    if chosen != -1 && hovering(79.0, 5.0, 16.0, 16.0, at) {
        if queue && !sel.entries.is_empty() {
            out.push(ids.remove + sel.cursor.rem_euclid(sel.entries.len() as i32));
        } else if !queue {
            out.push(ids.clear_fixed);
        }
    }
    if queue {
        if hovering(200.0, 0.0, 18.0, 18.0, at) {
            out.push(ids.clear_queue);
        }
        let size = sel.entries.len();
        let visible = if size > 9 { 8 } else { size };
        for i in 0..visible {
            if hovering(180.0 + (i % 3) as f32 * 18.0, 30.0 + (i / 3) as f32 * 18.0, 18.0, 18.0, at) {
                out.push(ids.remove + i as i32);
            }
        }
    }
    out
}

/// The page buttons: true when the press turned a page.
pub(crate) fn page_click(m: &Model, at: [f32; 2]) -> bool {
    let mut page = PAGE.lock().unwrap_or_else(|e| e.into_inner());
    if inside([13.0, 68.0, 29.0, 12.0], at) && *page > 0 {
        *page -= 1;
        return true;
    }
    if inside([46.0, 68.0, 29.0, 12.0], at) && *page < m.fab.len() / 9 {
        *page += 1;
        return true;
    }
    false
}

/// The picker's tooltips: (lines, on the left of the screen).
pub(crate) fn picker_tooltip(host: &dyn ClientHost, m: &Model, sel: &Selection, at: [f32; 2]) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    let queue = sel.queue;
    let page = page();
    let chosen = selected_drop(sel, m);
    let name = |i: usize| -> String {
        m.fab.get(i).map(|(id, n)| if *n > 1 { format!("{} x{n}", item_name(host, id)) } else { item_name(host, id) }).unwrap_or_default()
    };
    if hovering(180.0, 0.0, 18.0, 18.0, at) {
        let key = if queue { "hostilenetworks.gui.fab_mode.queue" } else { "hostilenetworks.gui.fab_mode.fixed" };
        lines.push(lang(host, key, key));
    }
    if chosen != -1 && hovering(79.0, 5.0, 16.0, 16.0, at) {
        if queue {
            lines.push(name(chosen as usize));
            lines.push(lang(host, "hostilenetworks.gui.queue_clear_current", "Click to clear current item"));
        } else {
            lines.push(lang(host, "hostilenetworks.gui.clear", "Click to clear selection"));
        }
    }
    if queue {
        if hovering(200.0, 0.0, 18.0, 18.0, at) {
            lines.push(lang(host, "hostilenetworks.gui.clear_queue", "Click to clear queue"));
        }
        let size = sel.entries.len();
        let overflow = size > 9;
        let visible = if overflow { 8 } else { size };
        for i in 0..visible {
            if hovering(180.0 + (i % 3) as f32 * 18.0, 30.0 + (i / 3) as f32 * 18.0, 16.0, 16.0, at) {
                let idx = sel.entries[i];
                if idx >= 0 && (idx as usize) < m.fab.len() {
                    lines.push(name(idx as usize));
                    lines.push(lang(host, "hostilenetworks.gui.queue_remove", "Click to remove from queue"));
                }
            }
        }
        if overflow && hovering(180.0 + 36.0, 30.0 + 36.0, 16.0, 16.0, at) {
            for j in visible..size {
                let idx = sel.entries[j];
                if idx >= 0 && (idx as usize) < m.fab.len() {
                    lines.push(name(idx as usize));
                }
            }
        }
    }
    for y in 0..3 {
        for x in 0..3 {
            let i = page * 9 + y * 3 + x;
            if i < m.fab.len() && hovering(18.0 + 18.0 * x as f32, 10.0 + 18.0 * y as f32, 16.0, 16.0, at) {
                let mut tip = vec![name(i)];
                if queue {
                    tip.push(lang(host, "hostilenetworks.gui.queue_add", "Click to add to queue"));
                }
                return (tip, true);
            }
        }
    }
    (lines, false)
}

pub(crate) fn draw(host: &dyn ClientHost, draw: &mut dyn DrawContext, data: &[i32], o: [f32; 2], mouse: [f32; 2]) {
    if data.len() < DATA_SLOTS {
        return;
    }
    let cfg = config::get(host);
    let tex = "hostilenetworks:gui/loot_fabricator";
    blit(draw, tex, o, 0.0, 0.0, 0.0, 0.0, 176.0, 83.0);
    let energy_h = (53.0 * wide(data, 2) as f32 / cfg.fab_power_cap.max(1) as f32).floor();
    blit(draw, tex, o, 6.0, 10.0 + 53.0 - energy_h, 0.0, 83.0, 7.0, energy_h);
    let prog_h = (35.0 * data[0] as f32 / 60.0).floor().clamp(0.0, 35.0);
    blit(draw, tex, o, 84.0, 23.0 + 35.0 - prog_h, 7.0, 83.0, 6.0, prog_h);
    blit(draw, "hostilenetworks:gui/default_gui", o, 0.0, 88.0, 0.0, 0.0, 176.0, 90.0);
    blit(draw, tex, o, -22.0, 0.0, 31.0, 83.0, 18.0, 18.0);
    icon(draw, REDSTONE_TEXTURES[data[1].clamp(0, 2) as usize], [o[0] - 21.0, o[1] + 1.0], None);
    if let Some(m) = menu_slot(host, 0).and_then(|s| models::stored(host, &s)) {
        let sel = viewed_selection(host, m);
        draw_picker(host, draw, o, m, &sel, mouse);
    } else {
        *PAGE.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }
}

pub(crate) fn click(host: &dyn ClientHost, data: &[i32], at: [f32; 2]) -> Vec<i32> {
    let mut out = Vec::new();
    if data.len() < DATA_SLOTS {
        return out;
    }
    if hovering(-22.0, 0.0, 18.0, 18.0, at) {
        out.push(2000 + (data[1].clamp(0, 2) + 1) % 3);
    }
    if let Some(m) = menu_slot(host, 0).and_then(|s| models::stored(host, &s)) {
        if page_click(m, at) {
            return out;
        }
        let sel = viewed_selection(host, m);
        out.extend(picker_clicks(m, &sel, at, &Ids { cycle: -2, clear_fixed: -1, clear_queue: -3, drop: 0, remove: 1000 }));
    }
    out
}

pub(crate) fn tooltip(host: &dyn ClientHost, data: &[i32], at: [f32; 2]) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    if data.len() < DATA_SLOTS {
        return (lines, false);
    }
    let cfg = config::get(host);
    if hovering(6.0, 10.0, 7.0, 53.0, at) {
        lines.push(lang_args(host, "hostilenetworks.gui.energy", "%s/%s FE", &[wide(data, 2).to_string(), cfg.fab_power_cap.to_string()]));
        lines.push(lang_args(host, "hostilenetworks.gui.fab_cost", "Energy cost: %s FE/t", &[cfg.fab_power_cost.to_string()]));
    }
    if hovering(-22.0, 0.0, 18.0, 18.0, at) {
        let key = format!("hostilenetworks.gui.redstone.{}", REDSTONE_KEYS[data[1].clamp(0, 2) as usize]);
        lines.push(lang(host, &key, &key));
    }
    if let Some(m) = menu_slot(host, 0).and_then(|s| models::stored(host, &s)) {
        let sel = viewed_selection(host, m);
        let (more, left) = picker_tooltip(host, m, &sel, at);
        if left {
            return (more, true);
        }
        lines.extend(more);
    }
    (lines, false)
}
