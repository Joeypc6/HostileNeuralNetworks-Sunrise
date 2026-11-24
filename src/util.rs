//! Small helpers every part uses: NBT keys, item ids, stack building, the redstone and failure states, text.

use mod_api::*;
use std::collections::HashMap;
use std::sync::OnceLock;

pub(crate) const NS: &str = "hostilenetworks";
pub(crate) type Pos = [i32; 3];

/// The compound's key type: the SDK's own, kept under this path for the port's modules.
pub(crate) use mod_api::JavaString;
/// A machine's fields: its saved compound, item slots and menu data.
pub(crate) use mod_api::item::containers::ModBlockEntityFields as Fields;

pub(crate) fn java(s: &str) -> JavaString {
    JavaString::from(s)
}

pub(crate) fn get_i(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&java(key)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Short(v)) => i32::from(*v),
        _ => default,
    }
}

pub(crate) fn set_i(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(java(key), Tag::Int(value));
}

pub(crate) fn get_s(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&java(key)) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

pub(crate) fn get_ints(nbt: &Compound, key: &str, len: usize) -> Vec<i32> {
    let mut out = match nbt.get(&java(key)) {
        Some(Tag::IntArray(a)) => a.to_vec(),
        Some(Tag::List(l)) => l.iter().map(|t| if let Tag::Int(v) = t { *v } else { 0 }).collect(),
        _ => Vec::new(),
    };
    out.resize(len, 0);
    out
}

pub(crate) fn set_ints(nbt: &mut Compound, key: &str, values: &[i32]) {
    nbt.insert(java(key), Tag::IntArray(values.to_vec().into()));
}

pub(crate) fn get_c<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Compound> {
    match nbt.get(&java(key)) {
        Some(Tag::Compound(c)) => Some(c),
        _ => None,
    }
}

// ---- Items by id

/// Item keys by id, looked up once (the API has no name-to-item lookup outside registration).
static IDS: OnceLock<HashMap<String, ItemKey>> = OnceLock::new();

pub(crate) fn item_ids(host: &dyn Host) -> &'static HashMap<String, ItemKey> {
    IDS.get_or_init(|| host.item_keys().into_iter().filter_map(|k| Some((k.id(host)?.to_owned(), k))).collect())
}

/// An item's key once any hook with a host has looked the items up (for the hooks given no host).
pub(crate) fn known_item(id: &str) -> Option<ItemKey> {
    IDS.get()?.get(id).copied()
}

pub(crate) fn item(host: &dyn Host, id: &str) -> Option<ItemKey> {
    item_ids(host).get(id).copied()
}

pub(crate) fn is(host: &dyn Host, stack: &ItemStack, id: &str) -> bool {
    item(host, id) == Some(stack.item())
}

pub(crate) fn stack_of(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(item(host, id)?, count.max(1))
}

pub(crate) fn max_stack(host: &dyn Host, stack: &ItemStack) -> u32 {
    host.max_stack_size(stack.item()).unwrap_or(64).max(1)
}

/// `ItemStack.isSameItemSameComponents`.
pub(crate) fn same(a: &ItemStack, b: &ItemStack) -> bool {
    a.is_stackable_with(b)
}

pub(crate) fn with_count(stack: &ItemStack, count: u32) -> Option<ItemStack> {
    let mut s = stack.clone();
    s.set_count(count).ok()?;
    Some(s)
}

/// Puts what fits of `stack` into `slot` (`insertInternal`: the slot's own limit, the stack's max size); the rest.
pub(crate) fn insert_into(host: &dyn Host, slot: &mut Option<ItemStack>, stack: ItemStack) -> Option<ItemStack> {
    let limit = max_stack(host, &stack);
    let room = match slot {
        None => limit.min(stack.count()),
        Some(have) if same(have, &stack) => limit.saturating_sub(have.count()).min(stack.count()),
        Some(_) => 0,
    };
    if room == 0 {
        return Some(stack);
    }
    match slot {
        Some(have) => {
            let n = have.count() + room;
            let _ = have.set_count(n);
        }
        None => *slot = with_count(&stack, room),
    }
    (room < stack.count()).then(|| with_count(&stack, stack.count() - room)).flatten()
}

/// Takes one from a slot.
pub(crate) fn shrink_one(slot: &mut Option<ItemStack>) {
    if let Some(s) = slot.take() {
        *slot = s.shrink(1);
    }
}

// ---- RedstoneState

/// RedstoneState: IGNORED, OFF_WHEN_POWERED, ON_WHEN_POWERED.
pub(crate) fn redstone_matches(state: i32, powered: bool) -> bool {
    match state {
        1 => !powered,
        2 => powered,
        _ => true,
    }
}

pub(crate) const REDSTONE_KEYS: [&str; 3] = ["ignored", "off_when_powered", "on_when_powered"];
/// RedstoneState.getResourceLocation: the vanilla textures its button shows.
pub(crate) const REDSTONE_TEXTURES: [&str; 3] =
    ["minecraft:textures/item/redstone.png", "minecraft:textures/block/redstone_torch_off.png", "minecraft:textures/block/redstone_torch.png"];

pub(crate) fn powered(host: &dyn Host, pos: Pos) -> bool {
    host.level(host.dimension()).is_some_and(|l| l.has_neighbor_signal(host, pos))
}

// ---- FailureState

pub(crate) const F_NONE: i32 = 0;
pub(crate) const F_OUTPUT: i32 = 1;
pub(crate) const F_ENERGY: i32 = 2;
pub(crate) const F_INPUT: i32 = 3;
pub(crate) const F_MODEL: i32 = 4;
pub(crate) const F_FAULTY: i32 = 5;
pub(crate) const F_ENERGY_MID: i32 = 6;
pub(crate) const F_REDSTONE: i32 = 7;
pub(crate) const F_CANNOT_TRAIN: i32 = 8;
pub(crate) const F_SHELL: i32 = 9;
pub(crate) const F_NOT_SELF_AWARE: i32 = 10;
pub(crate) const F_NO_SELECTION: i32 = 11;
pub(crate) const F_BUFFERING: i32 = 12;
pub(crate) const FAIL_NAMES: [&str; 13] = [
    "none", "output", "energy", "input", "model", "faulty", "energy_mid_cycle", "redstone", "cannot_train", "shell_broken",
    "not_self_aware", "no_selection", "buffering",
];

/// FailureState.BY_ID: out of range is NONE.
pub(crate) fn fail_key(state: i32) -> String {
    format!("hostilenetworks.fail.{}", FAIL_NAMES.get(state as usize).copied().unwrap_or("none"))
}

// ---- Menu data slots: 16 bits each on the network

pub(crate) fn put_wide(data: &mut [i32], lo: usize, v: i32) {
    data[lo] = v & 0xFFFF;
    data[lo + 1] = (v >> 16) & 0xFFFF;
}

pub(crate) fn wide(data: &[i32], lo: usize) -> i32 {
    (data[lo] & 0xFFFF) | ((data[lo + 1] & 0xFFFF) << 16)
}

// ---- Randomness (the level's random isn't on a block entity tick's host)

/// A float in [0, 1) from a small xorshift seeded by the clock and a position.
pub(crate) fn next_float(salt: Pos) -> f32 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static STATE: AtomicU64 = AtomicU64::new(0);
    let mut x = STATE.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9E37_79B9, |d| d.as_nanos() as u64) | 1;
    }
    x ^= (salt[0] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (salt[2] as u64).rotate_left(17) ^ salt[1] as u64;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, Ordering::Relaxed);
    (x >> 40) as f32 / (1u64 << 24) as f32
}

// ---- Text

/// The client's translation of `key`, else `english`.
pub(crate) fn lang(host: &dyn ClientHost, key: &str, english: &str) -> String {
    host.translate(key).unwrap_or_else(|| english.to_owned())
}

/// `key` translated with `args` put in its `%s` (and `%1$s`) places, `%%` as one percent sign.
pub(crate) fn lang_args(host: &dyn ClientHost, key: &str, english: &str, args: &[String]) -> String {
    format_args_java(&lang(host, key, english), args)
}

pub(crate) fn format_args_java(pattern: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('%') => {
                chars.next();
                out.push('%');
            }
            Some('s') | Some('d') => {
                chars.next();
                out.push_str(args.get(next).map_or("", String::as_str));
                next += 1;
            }
            Some(d) if d.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(d) = chars.peek().copied().filter(char::is_ascii_digit) {
                    n.push(d);
                    chars.next();
                }
                if chars.peek() == Some(&'$') {
                    chars.next();
                    chars.next();
                }
                let i = n.parse::<usize>().unwrap_or(1).saturating_sub(1);
                out.push_str(args.get(i).map_or("", String::as_str));
            }
            _ => out.push('%'),
        }
    }
    out
}

/// An ARGB colour from 0xRRGGBB.
pub(crate) const fn argb(rgb: u32) -> u32 {
    0xFF00_0000 | (rgb & 0xFF_FFFF)
}

/// ChatFormatting's colours by code.
pub(crate) const LEGACY: [(char, u32); 16] = [
    ('0', 0x000000), ('1', 0x0000AA), ('2', 0x00AA00), ('3', 0x00AAAA), ('4', 0xAA0000), ('5', 0xAA00AA),
    ('6', 0xFFAA00), ('7', 0xAAAAAA), ('8', 0x555555), ('9', 0x5555FF), ('a', 0x55FF55), ('b', 0x55FFFF),
    ('c', 0xFF5555), ('d', 0xFF55FF), ('e', 0xFFFF55), ('f', 0xFFFFFF),
];

/// The `§` code of the ChatFormatting colour nearest `rgb`, for tooltip lines that take codes.
pub(crate) fn code_for(rgb: u32) -> String {
    let d = |c: u32| {
        let p = |s: u32| (((c >> s) & 0xFF) as i32 - ((rgb >> s) & 0xFF) as i32).pow(2);
        p(16) + p(8) + p(0)
    };
    let best = LEGACY.iter().min_by_key(|(_, c)| d(*c)).map_or('f', |(code, _)| *code);
    format!("§{best}")
}

/// `Text` in colour `rgb` (a hex `TextColor`).
pub(crate) fn coloured(text: impl Into<String>, rgb: u32) -> Text {
    let mut style = mod_api::text::TextStyle::default();
    style.color = Some(format!("#{:06X}", rgb & 0xFF_FFFF));
    Text::literal(text.into()).with_style(style)
}

/// Words of `text` (its own line breaks kept) in lines of at most `width` GUI pixels.
pub(crate) fn wrap(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let tried = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if measure(&tried) <= width || line.is_empty() {
                line = tried;
            } else {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            }
        }
        out.push(line);
    }
    out
}

pub(crate) fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// AbstractContainerScreen.isHovering: a slot-sized box grown by one pixel each way.
pub(crate) fn hovering(x: f32, y: f32, w: f32, h: f32, at: [f32; 2]) -> bool {
    at[0] >= x - 1.0 && at[0] < x + w + 1.0 && at[1] >= y - 1.0 && at[1] < y + h + 1.0
}

/// The name of an item as the client's lang has it.
pub(crate) fn item_name(host: &dyn ClientHost, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    host.translate(&format!("item.{ns}.{path}"))
        .or_else(|| host.translate(&format!("block.{ns}.{path}")))
        .unwrap_or_else(|| path.replace('_', " "))
}

/// An entity type's name (`EntityType.getDescription`).
pub(crate) fn entity_name(host: &dyn ClientHost, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    host.translate(&format!("entity.{ns}.{path}")).unwrap_or_else(|| path.replace('_', " "))
}

/// A block's name (`Block.getName`).
pub(crate) fn block_name(host: &dyn ClientHost, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    host.translate(&format!("block.{ns}.{path}")).unwrap_or_else(|| path.replace('_', " "))
}

/// A line with `§` colour and style codes as a `Text` of coloured pieces.
pub(crate) fn legacy_text(s: &str) -> Text {
    let mut out: Option<Text> = None;
    let mut style = mod_api::text::TextStyle::default();
    let mut current = String::new();
    let mut push = |out: &mut Option<Text>, text: &str, style: &mod_api::text::TextStyle| {
        if text.is_empty() {
            return;
        }
        let piece = Text::literal(text.to_owned()).with_style(style.clone());
        *out = Some(match out.take() {
            None => Text::literal(String::new()).append(piece),
            Some(t) => t.append(piece),
        });
    };
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '§' {
            current.push(c);
            continue;
        }
        let Some(code) = chars.next() else { break };
        push(&mut out, &current, &style);
        current.clear();
        let code = code.to_ascii_lowercase();
        if let Some((_, rgb)) = LEGACY.iter().find(|(k, _)| *k == code) {
            style = mod_api::text::TextStyle::default();
            style.color = Some(format!("#{rgb:06X}"));
        } else {
            match code {
                'k' => style.obfuscated = Some(true),
                'l' => style.bold = Some(true),
                'm' => style.strikethrough = Some(true),
                'n' => style.underlined = Some(true),
                'o' => style.italic = Some(true),
                'r' => style = mod_api::text::TextStyle::default(),
                _ => {}
            }
        }
    }
    push(&mut out, &current, &style);
    out.unwrap_or_else(|| Text::literal(String::new()))
}

/// A slot of a screen at `position` (GUI pixels from the screen's corner), with no empty-slot icon: Sunrise's screens
/// take `ScreenSlot`s since 2d81263.
pub(crate) fn slot_at(position: [f32; 2]) -> ScreenSlot {
    ScreenSlot { position, no_item_icon: None }
}

/// A block entity tick on an owned copy of its fields: the saved compound, the item slots and the menu data, as the
/// tick hook handed them before it lent the resident entity in place. `tick` runs with the host, the entity's position
/// and type id, and the copy; what it changed is written back to the entity and the chunk is marked to save
/// (`setChanged`). A block entity gone between the two loans keeps nothing.
pub(crate) fn tick_on_fields(subject: &mut BlockEntityScope<'_>, tick: impl FnOnce(&mut dyn ServerHost, Pos, &str, &mut Fields)) {
    let Some((handle, pos, id, before)) =
        subject.with_view(|view| (view.handle(), view.position(), view.id().to_owned(), fields_of(&view)))
    else {
        return;
    };
    let mut after = before.clone();
    tick(subject.host(), pos, &id, &mut after);
    if after == before {
        return;
    }
    if subject.with_view(|mut view| write_fields(&mut view, &before, &after)) == Some(true) {
        let _ = handle.set_changed(subject.host());
    }
}

/// The entity's fields, copied out of the view.
pub(crate) fn fields_of(view: &BlockEntityView<'_>) -> Fields {
    Fields {
        nbt: view.nbt().clone(),
        items: (0..view.item_slots() as u32).map(|i| view.item_slot(i).and_then(|s| s.to_owned())).collect(),
        data: view.data_slots().to_vec(),
    }
}

/// Writes what changed from `before` to `after` into the view: true when anything was written.
pub(crate) fn write_fields(view: &mut BlockEntityView<'_>, before: &Fields, after: &Fields) -> bool {
    let mut wrote = false;
    for (key, value) in &after.nbt {
        if before.nbt.get(key) != Some(value) {
            wrote |= view.set_nbt(key.clone(), Some(value.clone()));
        }
    }
    for key in before.nbt.keys() {
        if !after.nbt.contains_key(key) {
            wrote |= view.set_nbt(key.clone(), None);
        }
    }
    for (slot, stack) in after.items.iter().enumerate().take(view.item_slots()) {
        if before.items.get(slot) != Some(stack) {
            wrote |= view.set_item_slot(slot as u32, stack.clone()).unwrap_or(false);
        }
    }
    if after.data != before.data {
        if after.data.len() == before.data.len() {
            for (slot, value) in after.data.iter().enumerate() {
                if before.data[slot] != *value {
                    wrote |= view.set_data_slot(slot as u32, *value);
                }
            }
        } else {
            wrote |= view.set_data_slots(&after.data);
        }
    }
    wrote
}
