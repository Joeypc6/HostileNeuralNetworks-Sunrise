//! Hostile Neural Networks (Shadows_of_Fire, MIT) for the arismoko/horizon fork's mod API: data models attuned with a
//! Model Framework and trained in a Deep Learner, the Simulation Chamber, the Loot Fabricator and the Data Center.
//! Written against Hooks, Parts and capabilities only (no mixins, no `[[inject]]`), so it builds on the prebuilt SDK.
//!
//! - `models.rs` (+ `models_data.rs`, generated): DataModelRegistry, ModelTierRegistry, DataModelInstance, components.
//! - `sim.rs`, `fab.rs`, `center.rs`: the three machines and the IO port; `items.rs`: their ITEMS and ENERGY handlers.
//! - `learner.rs`: the Deep Learner, its screen, HUD and tooltip; kills and mined blocks.
//! - `actions.rs`: attunement and the Fabrication Directive; `tooltips.rs`: names and hover text.
//! - `jei.rs`, `jade.rs`, `guide.rs`: the recipe viewer, Jade and the W guide.
//! - `commands.rs`, `commands_menu.rs`: `/hostilenetworks give_model` and the `/hostilenetworks` commands menu.

#![allow(dead_code)]

mod actions;
mod center;
mod commands;
mod commands_menu;
mod config;
mod fab;
mod gui;
mod items;
mod jade;
mod jei;
mod learner;
mod mobs;
mod models;
mod models_data;
mod sim;
mod tooltips;
mod util;

use mod_api::*;
use util::*;

pub(crate) const VERSION: &str = "7.0.0";

#[derive(Default)]
pub(crate) struct Mod;

/// The machines' blocks and block entities (one Part, dispatched by the block entity's type).
pub(crate) struct Machine;
/// The IO port.
pub(crate) struct Port;
/// The items with behaviour of their own.
pub(crate) struct Items;
/// The three menu types' slot rules.
pub(crate) struct Menus;

fn kind_of(id: &str) -> Option<items::Kind> {
    match id {
        sim::BLOCK => Some(items::Kind::Sim),
        fab::BLOCK => Some(items::Kind::Fab),
        center::BLOCK => Some(items::Kind::Center),
        _ => None,
    }
}

/// The server's numbers, sent at login (`config::NET_CONFIG`).
struct ConfigPayload;
impl IPayloadHandler for ConfigPayload {
    fn handle(&self, payload: PayloadBytes, _cx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        config::receive(&payload.0);
        Ok(())
    }
}

/// A Deep Learner screen request (`learner::NET_LEARNER`); the sender is the connection's player, never the bytes.
struct LearnerPayload;
impl IPayloadHandler for LearnerPayload {
    fn handle(&self, payload: PayloadBytes, cx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let Some(op) = learner::decode(&payload.0) else { return Ok(()) };
        let Ok(player) = cx.player() else { return Ok(()) };
        let uuid = player.uuid();
        if let Ok(host) = cx.server_host() {
            learner::request(host, uuid, op);
        }
        Ok(())
    }
}

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    /// The learner's tooltip component (`ClientRegistry`'s `RegisterClientTooltipComponentFactoriesEvent` listener).
    fn register_client_tooltip_component_factories(&self, _cx: &RegistrationContext, event: &mut RegisterClientTooltipComponentFactoriesEvent<'_>) {
        event.factories.register::<learner::LearnerTooltip, learner::LearnerTooltipClient>(learner::LearnerTooltipClient::new);
    }

    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        for (name, kind) in [("sim_chamber", items::Kind::Sim), ("loot_fabricator", items::Kind::Fab), ("data_center", items::Kind::Center)] {
            let block = format!("{NS}:{name}");
            let ty = r.block_entity_type(name).blocks(&[block.as_str()]).build();
            r.block_entity_part(&ty, Machine).expect("a native block entity type");
            r.block_entity_capability_before_game(&ITEMS, &ty, move |_level, at, _, _, offer| {
                offer.offer(|| items::machine_items(at, kind))
            });
            r.menu_part(&block, Menus);
            let _ = r.register_menu_screen(&block, std::sync::Arc::new(machine_screen));
        }
        let port = r.block_entity_type("data_center_io_port").blocks(&[center::PORT]).build();
        r.block_entity_part(&port, Port).expect("a native block entity type");
        r.block_entity_capability_before_game(&ITEMS, &port, items::port_items);
        r.block_entity_capability_before_game(&ENERGY, &port, items::port_energy);
        for item in ["deep_learner", "data_model", "prediction", "blank_data_model", "fab_directive", "sim_chamber", "loot_fabricator", "data_center", "data_center_io_port"] {
            let key = r.item_key(&format!("{NS}:{item}"));
            r.item_part(key, Items);
        }
        let _ = center::TICKETS.set(r.register_ticket_controller("data_center", true));
        // The machines' containers and energy buffers, once each (the old `block_entity_container` and
        // `block_entity_energy` hooks' answers).
        let cfg = config::at_startup(_cx);
        for (block, cap) in [(sim::BLOCK, cfg.sim_power_cap), (fab::BLOCK, cfg.fab_power_cap), (center::BLOCK, cfg.data_center_power_cap)] {
            let (slots, inputs, outputs): (usize, Vec<usize>, Vec<usize>) = match block {
                sim::BLOCK => (4, vec![0, 1], vec![2, 3]),
                fab::BLOCK => (17, vec![0], (1..17).collect()),
                _ => (45, (0..29).collect(), (25..45).collect()),
            };
            let all: Vec<usize> = (0..slots).collect();
            let key = r.block_key(block);
            r.block_container(&key, ModContainer { slots, faces: std::array::from_fn(|_| all.clone()), inputs, outputs, insert_rules: Vec::new(), menu: Some(block.to_owned()) });
            r.block_energy(&key, ModEnergyBuffer { capacity: cap, max_insert: cap, max_extract: 0, insert_faces: [true; 6], extract_faces: [false; 6] });
        }
        // `/hostilenetworks`: give_model's tier argument and the commands menu's channel.
        commands::register_arguments(r);
        commands_menu::register_payload(r);
        let mut payloads = r.payload_handlers();
        let mut reg = payloads.registrar("1");
        reg.play_to_client(PayloadId(config::NET_CONFIG.into()), Some(std::sync::Arc::new(ConfigPayload)));
        reg.play_to_server(PayloadId(learner::NET_LEARNER.into()), std::sync::Arc::new(LearnerPayload));
    }

    fn component_types(&self, _cx: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        for id in models::COMPONENT_TYPES {
            event.register((*id).to_owned());
        }
    }

    /// PredictionTintSource: the model's name colour.
    fn register_item_tint_sources(&self, _cx: &RegistrationContext, event: &mut RegisterItemTintSourcesEvent<'_>) {
        let Ok(id) = Identifier::parse("hostilenetworks:model_color") else { return };
        event.register(id, |_config| {
            Ok(|host: &dyn ClientHost, input: &ItemTintInput<'_>| {
                models::stored(host, input.stack).map_or(0xFFFF_FFFF, |m| 0xFF00_0000 | m.def.name_color)
            })
        });
    }

    /// DataModelItemModel: the frame (the item definition's base) and, drawn over it in screens, the mob
    /// (`item_decorations`). The node itself draws nothing.
    fn register_item_models(&self, _cx: &RegistrationContext, event: &mut RegisterItemModelsEvent<'_>) {
        let Ok(id) = Identifier::parse("hostilenetworks:data_model") else { return };
        event.register(id, |_config, _children| {
            Ok(|_host: &dyn ClientHost, _input: &ItemModelUpdateInput<'_>, layers: &mut Vec<ItemModelLayer>| layers.clear())
        });
    }

    fn register_gui_layers(&self, _cx: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        event.layers.register(
            "hostilenetworks:deep_learner".to_owned(),
            GuiLayerPlacement::AboveAll,
            Box::new(|host, draw| learner::hud(host, draw)),
        );
    }

    /// HostileEvents.cmds: `/hostilenetworks` (the commands menu) and `give_model` (`commands.rs`).
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        commands::register(event);
    }

    fn server_starting(&self, host: &mut dyn ServerHost, _event: &mut ServerStartingEvent) {
        config::reload(&*host);
        let _ = item_ids(&*host);
        models::refresh_filters(&*host);
    }

    /// New tags (`/reload`, a client's from its server): the hopper filters' tag inputs are read again.
    fn tags_updated(&self, _context: &dyn ReloadContext, _event: &mut TagsUpdatedEvent) {
        models::forget_filters();
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        let bytes = config::encode(&config::get(&*host));
        if let Some(player) = host.player(event.player) {
            let _ = player.send_payload(host, &PayloadId(config::NET_CONFIG.into()), PayloadBytes(bytes));
        }
    }

    fn client_player_logging_in(&self, host: &mut dyn ClientHost, _event: &mut ClientPlayerLoggingInEvent) {
        mod_api::if_installed!(host, jei, {
            crate::jei::register();
        });
        mod_api::if_installed!(host, jade, {
            crate::jade::register();
        });
    }

    fn client_player_logging_out(&self, _host: &mut dyn ClientHost, _event: &mut ClientPlayerLoggingOutEvent) {
        config::forget_synced();
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        let _ = item_ids(&*host);
        learner::client_tick(host);
        actions::send_pending(host);
    }

    /// HostileEvents.kill: the mob's kill credit, the player whose blow (or arrow) killed it.
    fn living_drops(&self, host: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        let causing = event.source.causing;
        if causing > 0 && event.kind != "minecraft:player" {
            let killer = host.players().server_players().into_iter().find(|p| p.entity().id() == causing as u32).map(|p| p.uuid());
            if let Some(player) = killer {
                let kind = event.kind.to_owned();
                learner::killed_once(host, player, event.entity, &kind);
            }
        }
        Flow::Continue
    }

    /// A player's melee blow killed a mob: the same kill credit, told by the game directly.
    fn mob_killed(&self, host: &mut dyn ServerHost, event: &mut MobKilledEvent<'_>) {
        let kind = event.kind.to_owned();
        learner::killed_once(host, event.player, event.entity, &kind);
    }

    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        let id = event.state.block().id(&*host).unwrap_or_default().to_owned();
        learner::mined(host, &id);
        Flow::Continue
    }

    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        actions::attune_block(host, event)
    }

    fn build_creative_mode_tab_contents(&self, host: HostSide, event: &mut BuildCreativeModeTabContentsEvent<'_>) {
        // Both sides build the tab (the server for its own creative reads): the same entries on each.
        match host {
            HostSide::Server(h) => actions::creative_tab(&*h, event),
            HostSide::Client(h) => actions::creative_tab(&*h, event),
        }
    }

    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        // A data model or prediction made without its name (`/give`): the first line names its mob anyway.
        let id = event.stack.item().id(host).unwrap_or_default();
        if (id == "hostilenetworks:data_model" || id == "hostilenetworks:prediction")
            && let Some(stack) = event.stack.project()
            && stack.get(&models::Raw("minecraft:item_name")).is_none()
            && let Some(color) = event.lines.color(0)
        {
            let _ = event.lines.set(0, Text::literal(tooltips::name(host, &stack)), color);
        }
    }

    fn render_tooltip_texture(&self, host: &dyn ClientHost, event: &mut RenderTooltipTextureEvent<'_>) {
        // HostileClient.tooltipTexture: the custom_tooltip_colors tag's fancy frame.
        if event.texture.is_none()
            && let Some(stack) = event.stack
            && let Ok(tag) = Identifier::parse("hostilenetworks:custom_tooltip_colors")
            && let Some(key) = host.tags().key(&tag)
            && host.tags().is_in_tag(stack.item(), key) == Some(true)
        {
            *event.texture = Identifier::parse("hostilenetworks:fancy").ok();
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if host.mod_screen() == Some(NS) {
            learner::draw(host, event.draw, event.screen.mouse);
            return;
        }
        actions::screen_post(host, event);
    }

    fn screen_mouse_button_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        if host.mod_screen() == Some(NS) {
            learner::press(host, event.screen.mouse, event.button);
            return Flow::Cancel;
        }
        actions::screen_press(host, event)
    }

    fn screen_key_pressed_pre(&self, host: &dyn ClientHost, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        if center::config_slot().is_some() && event.key == "key.keyboard.escape" {
            center::close_config();
            return Flow::Cancel;
        }
        let _ = host;
        Flow::Continue
    }

    fn screen_closing(&self, _host: &dyn ClientHost, _event: &mut ScreenClosingEvent<'_>) {
        center::close_config();
        sim::screen_opened();
        *fab::PAGE.lock().unwrap_or_else(|e| e.into_inner()) = 0;
    }

    fn validate_tickets(&self, host: &mut dyn ServerHost, event: &mut ValidateTicketsEvent<'_>) {
        actions::validate_tickets(host, event);
    }
}

#[mod_api::sunrise_part]
impl BlockPart for Machine {
    /// SimChamberBlock (and the others).getStateForPlacement: facing the placer.
    fn state_for_placement(&self, level: LevelSide, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockState>> {
        // A state change from the placer's facing: both sides answer it alike.
        let (LevelSide::Server(level) | LevelSide::Client(level)) = level;
        actions::facing_placement(level, event)
    }

    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        // LootFabBlock.useItemOn: the directive goes to its own useOn.
        let stack = event.click.stack?;
        if is(&*host, stack, "hostilenetworks:fab_directive") && level_block(&*host, event.click.pos).as_deref() == Some(fab::BLOCK) {
            return Some(InteractionResult::Pass);
        }
        None
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        // MenuUtil.openGui.
        let level = host.level(host.dimension())?;
        let be = host.block_entity(level, event.click.pos)?;
        let player = host.player(event.click.player)?;
        if let Some(block) = level_block(&*host, event.click.pos) {
            pad_slots(host, be, &block);
        }
        let _ = player.open_menu(host, be);
        Some(InteractionResult::Success)
    }
}

pub(crate) fn level_block(host: &dyn Host, pos: Pos) -> Option<String> {
    host.level(host.dimension())?.block(host, pos)?.id(host).map(str::to_owned)
}

#[mod_api::sunrise_part]
impl BlockEntityPart for Machine {
    fn block_entity(&self, _level: LevelSide, _event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Some(Compound::new())
    }

    /// The item handlers' isValid for the container's own path (`Container.canPlaceItem`: hoppers and droppers
    /// without the `ITEMS` part, `/loot insert`): SimItemHandler, FabItemHandler and DataCenterItemHandler.
    /// The `ITEMS` part (`items.rs`) applies the same rules; `ModContainer::inputs` keeps the outputs closed.
    fn can_place_item(&self, event: &mut CanPlaceItemEvent<'_>) -> Option<bool> {
        // Before the filters are made (a server start makes them) refuse rather than take anything.
        let Some(f) = models::filters() else { return Some(false) };
        let (stack, items) = (event.stack, &event.entity.items);
        let model_at = |i: usize| items.get(i).and_then(Option::as_ref);
        let item = Some(stack.item());
        Some(match (event.entity.id(), event.slot) {
            (sim::BLOCK, 0) => item == f.data_model,
            (sim::BLOCK, 1) => model_at(0).is_some_and(|m| f.matches_input(m, stack)),
            (fab::BLOCK, 0) => item == f.prediction,
            (center::BLOCK, 0..=24) => item == f.data_model,
            (center::BLOCK, 25..=28) => (0..25).filter_map(model_at).any(|m| f.matches_input(m, stack)),
            _ => true,
        })
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, event: &mut BlockEntityTickEvent<'_>) {
        tick_on_fields(&mut event.subject, |host, pos, id, entity| {
            // The hopper filters (`can_place_item`), read again after tags changed.
            if models::filters().is_none() {
                models::refresh_filters(&*host);
            }
            match id {
                sim::BLOCK => sim::tick(host, pos, entity),
                fab::BLOCK => fab::tick(host, pos, entity),
                center::BLOCK => center::tick(host, pos, entity),
                _ => {}
            }
        });
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        // getUpdateTag: what the screens and the renderer read.
        let nbt = &event.entity.nbt;
        let mut tag = Compound::new();
        for key in ["saved_selections", "activeMask", "shellValid", "wallFaceOrd", "shellMinX", "shellMinY", "shellMinZ"] {
            if let Some(v) = nbt.get(&java(key)) {
                tag.insert(java(key), v.clone());
            }
        }
        Some(tag)
    }


    fn pre_remove_side_effects(&self, _level: &mut dyn LevelRandomHost, event: &mut PreRemoveSideEffectsEvent<'_>) {
        // The Data Center's buffered outputs (Java loses them; the fork keeps them on the block entity) drop with it.
        let _ = event;
    }
}

#[mod_api::sunrise_part]
impl BlockPart for Port {
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        center::cycle_port(host, event.click)
    }
}

#[mod_api::sunrise_part]
impl BlockEntityPart for Port {
    fn block_entity(&self, _level: LevelSide, _event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Some(Compound::new())
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        let mut tag = Compound::new();
        if let Some(v) = event.entity.nbt.get(&java("owner")) {
            tag.insert(java("owner"), v.clone());
        }
        Some(tag)
    }
}

#[mod_api::sunrise_part]
impl MenuPart for Menus {
    /// Every id of the three machines' screens reaches `menu_button`, as every `clickMenuButton` reached the Java
    /// menus (each checks its own ids). Main drops a button nobody accepts, which is why the fabricator's drop pick
    /// and the data center's config buttons did nothing.
    fn menu_button_valid(&self, _host: &dyn Host, event: &mut MenuButtonValidEvent<'_>) -> Flow {
        match event.block {
            Some(sim::BLOCK | fab::BLOCK | center::BLOCK) => Flow::Cancel,
            _ => Flow::Continue,
        }
    }

    fn menu_button(&self, event: &mut MenuButtonEvent<'_>) {
        let Some(pos) = event.context.pos else { return };
        let id = event.context.id;
        // The machine's fields, worked on as a copy and written back through the scoped view where they changed.
        let Some((handle, kind, before)) = event.subject.with_view(|v| (v.handle(), v.id().to_owned(), util::fields_of(&v))) else { return };
        let mut entity = before.clone();
        let host = event.subject.host();
        let sync = match kind.as_str() {
            sim::BLOCK => {
                sim::button(&*host, &mut entity.nbt, id);
                false
            }
            fab::BLOCK => fab::button(&*host, pos, &mut entity, id),
            center::BLOCK => center::button(&*host, &mut entity, id),
            _ => false,
        };
        // setChanged, as the machine's own setters did.
        if entity != before && event.subject.with_view(|mut v| util::write_fields(&mut v, &before, &entity)) == Some(true) {
            let _ = handle.set_changed(event.subject.host());
        }
        if sync {
            event.subject.host().block_entity_sync(pos);
        }
    }

    /// The FilteredSlots' rules, the inputs by the models in the menu's slots.
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        actions::slot_rules(host, event);
    }
}

#[mod_api::sunrise_part]
impl ItemPart for Items {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        actions::use_item(host, event)
    }

    /// Server only, as before: the client defers; the old claim swung on the server (SuccessServer).
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => match actions::use_on(host, event) {
                Flow::Cancel => Some(InteractionResult::SuccessServer),
                _ => None,
            },
            HostSide::Client(_) => None,
        }
    }

    fn tooltip(&self, host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let mut lines = Vec::new();
        tooltips::append(host, event.stack, &mut lines);
        for l in lines {
            event.lines.push((legacy_text(&l), u32::MAX));
        }
    }

    /// `DeepLearnerItem.getTooltipImage`: the learner's model bars.
    fn tooltip_image(&self, host: &dyn ClientHost, event: &mut TooltipImageEvent<'_>) -> Option<TooltipComponent> {
        if is(host, event.stack, learner::LEARNER) { learner::tooltip_image(host, event.stack) } else { None }
    }

    /// A data model or prediction made elsewhere (`/give`, another mod) gets its mob's name, as every one this mod
    /// makes has (`getName`), so the hotbar, chat and other mods show it.
    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        actions::name_stack(host, event.stack, event.slot);
    }

    /// HostileEvents.modelAttunement: a Model Framework used on a mob.
    /// Server only, as before; the old `Some(false)` owned without a swing (Consume), `Some(true)` swung (SuccessServer).
    fn item_interact_mob(&self, host: HostSide, event: &mut ItemInteractMobEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => actions::attune_entity(host, event).map(|swing| if swing { InteractionResult::SuccessServer } else { InteractionResult::Consume }),
            HostSide::Client(_) => None,
        }
    }

    fn item_decorations(&self, host: &dyn ClientHost, event: &mut ItemDecorationsEvent<'_>) {
        actions::decorate(host, event);
    }
}

// ---- The machines' screens (client), by menu type: the menu type is the machine block's id.

/// The screen for one opening of a machine's menu (`register_menu_screen`), drawn, clicked and hovered from the menu's
/// live data.
fn machine_screen(_host: &dyn ClientHost, open: MenuScreenOpen<'_>) -> Option<MenuScreen> {
    // Only a menu opened for one of HNN's machines has these screens.
    let block = open.block?.to_owned();
    let layout = match block.as_str() {
        sim::BLOCK => sim::screen(),
        fab::BLOCK => fab::screen(),
        center::BLOCK => center::screen(),
        _ => return None,
    };
    let (on_draw, on_click, on_tooltip) = (block.clone(), block.clone(), block);
    Some(
        MenuScreen::new(layout)
            .with_draw(move |host, frame, draw| screen_draw(host, &on_draw, frame.data, frame.origin, draw))
            .with_click(move |host, frame, right| screen_click(host, &on_click, frame.data, frame.at, right))
            .with_tooltip(move |host, frame, _slot, lines| screen_tooltip(host, &on_tooltip, frame.data, frame.at, lines)),
    )
}

fn screen_draw(host: &dyn ClientHost, block: &str, data: &[i32], origin: [f32; 2], draw: &mut dyn DrawContext) {
    let mouse = gui::with_shown(|s| {
        if s.block != block {
            s.block = block.to_owned();
        }
        s.data = data.to_vec();
        s.origin = origin;
        s.mouse
    });
    let mouse_abs = host.cursor();
    let _ = mouse;
    match block {
        sim::BLOCK => sim::draw(host, draw, data, origin),
        fab::BLOCK => fab::draw(host, draw, data, origin, mouse_abs),
        center::BLOCK => center::draw(host, draw, data, origin),
        _ => {}
    }
}

fn screen_click(host: &dyn ClientHost, block: &str, data: &[i32], at: [f32; 2], right: bool) -> Option<i32> {
    if center::config_slot().is_some() {
        return None;
    }
    match block {
        sim::BLOCK if !right => sim::click(host, data, at),
        fab::BLOCK if !right => fab::click(host, data, at).into_iter().next(),
        center::BLOCK if !right => center::click(data, at),
        _ => None,
    }
}

fn screen_tooltip(host: &dyn ClientHost, block: &str, data: &[i32], at: [f32; 2], out: &mut Vec<(Text, u32)>) {
    gui::with_shown(|s| s.mouse = at);
    if center::config_slot().is_some() {
        return;
    }
    let styled = |lines: Vec<String>| lines.into_iter().map(|l| (util::legacy_text(&l), u32::MAX));
    match block {
        sim::BLOCK => {
            let mut lines = Vec::new();
            sim::tooltip(host, data, at, &mut lines);
            out.extend(styled(lines));
        }
        fab::BLOCK => {
            let (lines, left) = fab::tooltip(host, data, at);
            if left {
                gui::with_shown(|s| s.data = data.to_vec());
                actions::set_left_tooltip(lines);
            } else {
                out.extend(styled(lines));
            }
        }
        center::BLOCK => {
            let (lines, left) = center::tooltip(host, data, at);
            out.extend(styled(lines));
            if !left.is_empty() {
                actions::set_left_tooltip(left);
            }
        }
        _ => {}
    }
}

/// Workaround for sunrise-org/sunrise#4463: a new machine holds no item slots until one is written, and main sizes its
/// menu by the slots it holds, so a shift-click into a fresh machine was refused (or mapped to the wrong inventory
/// slot). Writing the last declared slot once and clearing it makes the block entity hold all of them.
fn pad_slots(host: &mut dyn ServerHost, entity: BlockEntityRef, block: &str) {
    let slots: u32 = match block {
        sim::BLOCK => 4,
        fab::BLOCK => 17,
        center::BLOCK => 45,
        _ => return,
    };
    let last = slots - 1;
    if !matches!(entity.item_slot(&*host, last), Ok(None)) {
        return;
    }
    let Some(filler) = util::stack_of(&*host, "minecraft:stone", 1) else { return };
    let _ = host.transaction(|host, tx| {
        entity.set_item_slot(host, tx, last, Some(filler)).map_err(|_| ())?;
        entity.set_item_slot(host, tx, last, None).map_err(|_| ())?;
        Ok::<(), ()>(())
    });
}
