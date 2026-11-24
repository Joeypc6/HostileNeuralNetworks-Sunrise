//! What players do with the items and the screens' extra input: HostileEvents' attunement, the Fabrication Directive,
//! the creative tab's models, the machines' placement and slot rules, the Data Center's configuration screen and the
//! tooltips the Java screens draw left of themselves.

use crate::center;
use crate::fab;
use crate::gui;
use crate::learner;
use crate::models::{self, Model};
use crate::util::*;
use crate::{config, sim};
use mod_api::*;
use std::sync::Mutex;

// ---- Attunement (HostileEvents.modelAttunement, modelAttunementBlock)

fn tell(host: &mut dyn ServerHost, player: u128, line: Text) {
    if let Some(p) = host.player(player) {
        let _ = p.tell(host, line);
    }
}

fn red(t: Text) -> Text {
    let mut style = mod_api::text::TextStyle::default();
    style.color = Some("red".to_owned());
    t.with_style(style)
}

fn model_name_text(m: &Model) -> Text {
    match models::item_name_tag("x", m) {
        Tag::Compound(c) => match c.get(&java("with")) {
            Some(Tag::List(l)) => match l.first() {
                Some(Tag::Compound(arg)) => match (get_s(arg, "translate"), get_s(arg, "text")) {
                    (Some(k), _) => Text::translate(k, Vec::new()),
                    (None, Some(t)) => Text::literal(t),
                    _ => Text::literal(m.def.source.to_owned()),
                },
                _ => Text::literal(m.def.source.to_owned()),
            },
            _ => Text::literal(m.def.source.to_owned()),
        },
        _ => Text::literal(m.def.source.to_owned()),
    }
}

/// pickMatch and completeAttunement: one model for the target, or a message.
fn attune(host: &mut dyn ServerHost, player: u128, hand: Hand, block: bool, kind: &str) {
    let candidates: Vec<&Model> = models::table(&*host).models.values().filter(|m| m.def.is_block == block && m.matches(kind)).collect();
    match candidates.as_slice() {
        [] => tell(host, player, red(Text::translate("hostilenetworks.msg.no_model", Vec::new()))),
        [m] => {
            let Some(stack) = models::data_model(&*host, m) else { return };
            let mut gold = mod_api::text::TextStyle::default();
            gold.color = Some("gold".to_owned());
            tell(host, player, Text::translate("hostilenetworks.msg.built", vec![model_name_text(m)]).with_style(gold));
            // setItemInHand: the hand's slot itself (a creative hand would keep the framework and add the model).
            let slot = match hand {
                Hand::Main => host.player(player).and_then(|p| p.selected_slot(&*host)).unwrap_or(0) as i32,
                Hand::Off => -106,
            };
            let place = ItemPlace::PlayerSlot { player, slot };
            let _ = host.transaction(|host, tx| {
                let Some(old) = place.with_stack(host, |s| s.project()).flatten() else { return Err(()) };
                if place.extract(host, tx, &ItemResource::of(&old), old.count()).unwrap_or(0) != old.count() {
                    return Err(());
                }
                if place.insert(host, tx, &ItemResource::of(&stack), 1).unwrap_or(0) != 1 {
                    return Err(());
                }
                Ok(())
            });
        }
        [a, b, ..] => {
            let line = Text::translate("hostilenetworks.msg.multiple_models", vec![model_name_text(b), model_name_text(a)]);
            tell(host, player, red(line));
        }
    }
}

pub(crate) fn attune_entity(host: &mut dyn ServerHost, event: &mut ItemInteractMobEvent<'_>) -> Option<bool> {
    if !is(&*host, event.stack, "hostilenetworks:blank_data_model") || !config::get(&*host).right_click_to_attune {
        return None;
    }
    let player = host.players().acting_player()?;
    let uuid = player.uuid();
    let main = ItemPlace::PlayerHand { player: uuid, hand: Hand::Main }.with_stack(host, |s| s.project()).flatten();
    let hand = if main.as_ref().is_some_and(|s| is(&*host, s, "hostilenetworks:blank_data_model")) { Hand::Main } else { Hand::Off };
    let kind = event.kind.to_owned();
    attune(host, uuid, hand, false, &kind);
    Some(false)
}

pub(crate) fn attune_block(host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
    let click = event.click;
    match host {
        HostSide::Server(host) => {
            if !config::get(&*host).right_click_to_attune {
                return Flow::Continue;
            }
            let Some(stack) = click.stack else { return Flow::Continue };
            if !is(&*host, stack, "hostilenetworks:blank_data_model") {
                return Flow::Continue;
            }
            let Some(block) = crate::level_block(&*host, click.pos) else { return Flow::Continue };
            attune(host, click.player, click.hand, true, &block);
            *event.cancellation_result = InteractionResult::Consume;
            Flow::Cancel
        }
        HostSide::Client(host) => {
            let held = click.stack.is_some_and(|s| is(&*host, s, "hostilenetworks:blank_data_model"));
            if held && config::get(&*host).right_click_to_attune {
                *event.cancellation_result = InteractionResult::Consume;
                return Flow::Cancel;
            }
            Flow::Continue
        }
    }
}

// ---- The Deep Learner and the Fabrication Directive

/// DeepLearnerItem.use: its screen (the client draws it).
pub(crate) fn use_item(host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
    match host {
        HostSide::Client(host) => {
            if is(&*host, event.stack, learner::LEARNER) {
                learner::open(host);
                return Some(InteractionResult::Success);
            }
            None
        }
        HostSide::Server(host) => is(&*host, event.stack, learner::LEARNER).then_some(InteractionResult::Success),
    }
}

/// FabDirectiveItem.useOn: on a Loot Fabricator, copies its selections (and redstone state), or with sneaking
/// applies them.
pub(crate) fn use_on(host: &mut dyn ServerHost, event: &mut UseOnEvent<'_>) -> Flow {
    if !is(&*host, event.stack, "hostilenetworks:fab_directive") {
        return Flow::Continue;
    }
    if crate::level_block(&*host, event.pos).as_deref() != Some(fab::BLOCK) {
        return Flow::Continue;
    }
    let Some(level) = host.level(host.dimension()) else { return Flow::Continue };
    let Some(be) = host.block_entity(level, event.pos) else { return Flow::Continue };
    let player = event.player;
    let sneaking = host.player(player).is_some_and(|p| {
        let e = p.entity();
        e.is_shift_key_down(&*host) == Some(true) || e.is_crouching(&*host) == Some(true)
    });
    let mint = |t: Text| {
        let mut s = mod_api::text::TextStyle::default();
        s.color = Some("#00FFC0".to_owned());
        t.with_style(s)
    };
    if sneaking {
        let Some(Tag::Compound(saved)) = models::component(event.stack, models::FAB_SELECTIONS) else { return Flow::Cancel };
        let sels = get_c(&saved, "selections").cloned().unwrap_or_default();
        let redstone = match get_s(&saved, "redstone_state").as_deref() {
            Some("off_when_powered") => 1,
            Some("on_when_powered") => 2,
            _ => 0,
        };
        if sels.is_empty() && redstone == 0 {
            return Flow::Cancel;
        }
        let mut data = Compound::new();
        let mut converted = Compound::new();
        for (k, v) in &sels {
            converted.insert(k.clone(), fab::Selection::from_tag(v).to_tag());
        }
        let n = converted.len();
        data.insert(java("saved_selections"), Tag::Compound(converted));
        set_i(&mut data, "redstoneState", redstone);
        // setSelections replaces them all.
        let mut whole = be.data(&*host).unwrap_or_default();
        whole.extend(data);
        let _ = be.replace_data(host, whole);
        let name = Text::translate("item.hostilenetworks.fab_directive", Vec::new());
        tell(host, player, mint(Text::translate("text.hostilenetworks.selections_applied", vec![Text::literal(n.to_string()), name])));
    } else {
        let nbt = be.data(&*host).unwrap_or_default();
        let sels = fab::read_selections(&nbt);
        let mut map = Compound::new();
        for (k, v) in &sels {
            map.insert(java(k), v.to_tag());
        }
        let mut saved = Compound::new();
        saved.insert(java("selections"), Tag::Compound(map));
        saved.insert(java("redstone_state"), Tag::String(REDSTONE_KEYS[get_i(&nbt, "redstoneState", 0).clamp(0, 2) as usize].into()));
        let mut new = event.stack.clone();
        models::set_component(&mut new, models::FAB_SELECTIONS, Tag::Compound(saved));
        let slot = match event.hand {
            Hand::Main => host.player(player).and_then(|p| p.selected_slot(&*host)).unwrap_or(0) as i32,
            Hand::Off => -106,
        };
        let place = ItemPlace::PlayerSlot { player, slot };
        let old = event.stack.clone();
        let _ = host.transaction(|host, tx| {
            if place.extract(host, tx, &ItemResource::of(&old), 1).unwrap_or(0) != 1 || place.insert(host, tx, &ItemResource::of(&new), 1).unwrap_or(0) != 1 {
                return Err(());
            }
            Ok(())
        });
        let name = Text::translate("block.hostilenetworks.loot_fabricator", Vec::new());
        tell(host, player, mint(Text::translate("text.hostilenetworks.selections_copied", vec![Text::literal(sels.len().to_string()), name])));
    }
    Flow::Cancel
}

// ---- The creative tab (DataModelItem and MobPredictionItem fill it with every model's)

pub(crate) fn creative_tab(host: &dyn Host, event: &mut BuildCreativeModeTabContentsEvent<'_>) {
    if event.tab != "hostilenetworks:tab" {
        return;
    }
    let table = models::table(host);
    let mut extra: Vec<ItemStack> = table.models.values().filter_map(|m| models::data_model(host, m)).collect();
    extra.extend(table.models.values().filter_map(|m| models::prediction(host, m, 1)));
    // Right after the Deep Learner, in order (each after the one before), in the tab and in search; at the end
    // when the tab has no learner.
    let entries = event.edit.entries(host, CreativeTabList::Parent).unwrap_or_default();
    let mut anchor = entries.into_iter().find(|s| is(host, s, learner::LEARNER));
    for stack in extra {
        let placement = match &anchor {
            Some(a) => CreativeTabPlacement::After(a.clone()),
            None => CreativeTabPlacement::Last,
        };
        if event.edit.insert(host, placement, stack.clone(), CreativeTabVisibility::Both).is_ok() {
            anchor = Some(stack);
        }
    }
}

// ---- Placement and slots

/// getStateForPlacement: facing the placer (getHorizontalDirection().getOpposite()).
pub(crate) fn facing_placement(level: &dyn LevelHost, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockState>> {
    let facing = match event.context.source {
        PlacementSource::Player { yaw, .. } => {
            let yaw = yaw.rem_euclid(360.0);
            // The direction the player looks: south 0, west 90, north 180, east 270; the block faces back at them.
            let looking = ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4);
            ["north", "east", "south", "west"][looking as usize]
        }
        // DirectionalPlaceContext.getHorizontalDirection: its direction, or north when vertical; then the opposite.
        PlacementSource::Directional { direction } => match direction {
            Direction::South => "north",
            Direction::West => "east",
            Direction::East => "west",
            _ => "south",
        },
        _ => "north",
    };
    Some(level.state_with_property(event.state, "facing", facing))
}

/// The FilteredSlots. The inputs read the menu's own slots (`MenuSlotContext`, the same on the server and the
/// client's prediction): the chamber's input slot takes its model's input (DataModelItem.matchesModelInput with slot
/// 0), the Data Center's input slots what any installed model takes (isAnyInstalledModelInput).
/// How many slots a machine's menu has of its own (its container, `lib.rs`'s `block_container`).
fn machine_slots(menu: &str) -> usize {
    match menu {
        sim::BLOCK => 4,
        fab::BLOCK => 17,
        center::BLOCK => 45,
        _ => 0,
    }
}

pub(crate) fn slot_rules(host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
    // The machine's own slots only: the player's inventory slots are asked too (`MenuContainer::Player`).
    if event.container != MenuContainer::Main {
        return;
    }
    let item = event.stack.item();
    // By the host's id map, made here if nothing asked for it yet: a world's first click can come before any other
    // hook made it (`known_item` would read none and refuse every model).
    let is = |id: &str| crate::util::item(host, id) == Some(item);
    let model = is("hostilenetworks:data_model");
    // The machine's slots as the menu holds them (its rules read the models and inputs there).
    let slots: Vec<Option<ItemStack>> = (0..machine_slots(event.menu)).map(|i| event.context.slot(MenuContainer::Main, i)).collect();
    let slots = slots.as_slice();
    event.may_place = match (event.menu, event.slot) {
        (sim::BLOCK, 0) => model,
        (sim::BLOCK, 1) => sim::valid(host, slots, 1, event.stack),
        (sim::BLOCK, 2..=3) => false,
        (fab::BLOCK, 0) => is("hostilenetworks:prediction"),
        (fab::BLOCK, 1..=16) => false,
        (center::BLOCK, 0..=24) => model,
        (center::BLOCK, 25..=28) => center::valid(host, slots, event.slot, event.stack),
        (center::BLOCK, 29..=44) => false,
        _ => return,
    };
}

// ---- Chunk tickets (DataCenterTileEntity.validateLoadedTickets)

pub(crate) fn validate_tickets(host: &mut dyn ServerHost, event: &mut ValidateTicketsEvent<'_>) {
    if Some(event.controller) != center::TICKETS.get().copied() {
        return;
    }
    let Some(level) = host.level(event.dimension) else { return };
    for (owner, _set) in event.validation.block_tickets() {
        let is_center = level.block(&*host, owner).is_some_and(|b| b.id(&*host) == Some(center::BLOCK));
        if !is_center {
            event.validation.remove_block_tickets(owner);
        }
    }
}

// ---- The screens' extra drawing and input

static LEFT_TIP: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Menu buttons the overlay pressed, sent at the next client tick (the press hook can't send).
pub(crate) static PENDING: Mutex<Vec<i32>> = Mutex::new(Vec::new());

pub(crate) fn set_left_tooltip(lines: Vec<String>) {
    *LEFT_TIP.lock().unwrap_or_else(|e| e.into_inner()) = lines;
}

pub(crate) fn send_pending(host: &mut dyn ClientHost) {
    let ids: Vec<i32> = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
    for id in ids {
        gui::press(host, id);
    }
}

pub(crate) fn screen_post(host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
    let block = gui::with_shown(|s| s.block.clone());
    if !block.starts_with("hostilenetworks:") || host.screen_pos().is_none() {
        return;
    }
    let left = std::mem::take(&mut *LEFT_TIP.lock().unwrap_or_else(|e| e.into_inner()));
    if let Some(slot) = center::config_slot() {
        center::draw_config(host, event.draw, slot, event.screen.mouse);
        return;
    }
    if !left.is_empty() {
        let origin = gui::with_shown(|s| s.origin);
        let top = if block == fab::BLOCK { origin[1] + 15.0 } else { origin[1] + 40.0 };
        gui::tooltip_on_left(event.draw, &left, top, origin[0].min(240.0) - 4.0);
    }
}

pub(crate) fn screen_press(host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
    if host.screen_pos().is_none() {
        return Flow::Continue;
    }
    let (block, origin) = gui::with_shown(|s| (s.block.clone(), s.origin));
    let mouse = event.screen.mouse;
    if block == center::BLOCK {
        if let Some(slot) = center::config_slot() {
            if event.button == 0 {
                let (ids, close) = center::config_click(host, slot, mouse);
                if close {
                    center::close_config();
                }
                PENDING.lock().unwrap_or_else(|e| e.into_inner()).extend(ids);
            }
            return Flow::Cancel;
        }
        if event.button == 1 && center::right_press(host, [mouse[0] - origin[0], mouse[1] - origin[1]]) {
            return Flow::Cancel;
        }
    }
    Flow::Continue
}

// ---- A data model's icon: its mob over the frame (DataModelItemModel)

pub(crate) fn decorate(host: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
    if !is(host, event.stack, "hostilenetworks:data_model") {
        return;
    }
    let Some(m) = models::stored(host, event.stack) else { return };
    // DataModelGhostRenderer, GUI: the mob at 0.4 of its display scale (an icon is 16 GUI pixels a block), its feet
    // 0.32 of the icon below the middle, turned 30 degrees. A big mob may reach past the slot, as in Java.
    let (x, y) = (event.x, event.y);
    let px = 16.0 * 0.4 * m.def.scale;
    if crate::mobs::draw_entity(event.draw, m, [x + 8.0, y + 8.0 + 0.32 * 16.0], px, -30.0, [x - 24.0, y - 48.0, x + 40.0, y + 20.0]) {
        return;
    }
    let Some(key) = crate::mobs::picture_item(host, m) else { return };
    event.draw.item(key, [x + 3.0, y + 1.0], 10.0);
}

pub(crate) fn name_stack(host: &mut dyn ServerHost, stack: &ItemStack, slot: i32) {
    if models::component(stack, "minecraft:item_name").is_some() {
        return;
    }
    let item_id = if is(&*host, stack, "hostilenetworks:data_model") {
        "hostilenetworks:data_model"
    } else if is(&*host, stack, "hostilenetworks:prediction") {
        "hostilenetworks:prediction"
    } else {
        return;
    };
    let Some(m) = models::stored(&*host, stack) else { return };
    let Some(player) = host.players().acting_player() else { return };
    let mut named = stack.clone();
    models::set_component(&mut named, "minecraft:item_name", models::item_name_tag(item_id, m));
    let place = ItemPlace::PlayerSlot { player: player.uuid(), slot };
    let old = stack.clone();
    let _ = host.transaction(|host, tx| {
        if place.extract(host, tx, &ItemResource::of(&old), old.count()).unwrap_or(0) != old.count() {
            return Err(());
        }
        if place.insert(host, tx, &ItemResource::of(&named), named.count()).unwrap_or(0) != named.count() {
            return Err(());
        }
        Ok(())
    });
}
