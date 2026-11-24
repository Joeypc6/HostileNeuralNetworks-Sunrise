//! HostileEvents.cmds: the `/hostilenetworks` tree. GiveModelCommand's `give_model` is here; the bare
//! `/hostilenetworks` (which upstream left without an executor) opens the commands menu (`commands_menu.rs`).
//!
//! GenerateModelCommand's `generate_model_json`, `update_model_json`, `generate_all` and `datafix_all` are not
//! registered: they write data model files by killing a fresh, unspawned entity 2500 times through
//! `LivingEntity.dropFromLootTable` with the player as killer, and by re-reading the data packs' raw
//! `data_models/*.json` (`DataModelRegistry.prepare`). Sunrise's mod API has neither an entity death loot roll with a
//! killer and damage source (`level_roll_loot` gives the `fish`-style context: origin, tool, this entity) nor the raw
//! JSON of a data pack file, and this port's models are compiled in (`models_data.rs`).

use crate::models;
use crate::models_data::TIERS;
use mod_api::*;

/// The `tier` argument (`StringArgumentType.word()`, completed with ModelTierRegistry's ids' paths): any word
/// parses, so an unknown tier is the executor's "Invalid model tier" as upstream, not a parse error.
pub(crate) struct ModelTierArgument;

impl CommandArgumentType for ModelTierArgument {
    fn kind(&self) -> CommandArgumentKind {
        CommandArgumentKind::Word
    }

    fn parse(&self, input: &str) -> Result<String, CommandArgumentError> {
        Ok(input.to_owned())
    }

    fn suggest(&self, _sender: Option<&CommandSender>, prefix: &str) -> Vec<CommandSuggestion> {
        TIERS.iter().map(|t| t.name).filter(|n| n.starts_with(prefix)).map(CommandSuggestion::new).collect()
    }
}

/// Called from `register`.
pub(crate) fn register_arguments(r: &mut dyn Registrar) {
    r.register_command_argument("model_tier", ModelTierArgument);
}

/// The usages, for `register_commands`. GiveModelCommand requires `Commands.LEVEL_GAMEMASTERS` (2).
pub(crate) fn register(event: &mut RegisterCommandsEvent<'_>) {
    event.commands.push(crate::commands_menu::command());
    event.commands.push(
        Command::op("hostilenetworks give_model <targets:players> <model:id> <tier:hostilenetworks:model_tier> [data:integer]", 2)
            .executes(give_model),
    );
}

fn red(line: Text) -> Text {
    let mut style = mod_api::text::TextStyle::default();
    style.color = Some("red".to_owned());
    line.with_style(style)
}

/// GiveModelCommand.giveItem.
fn give_model(host: &mut dyn ServerHost, _source: &CommandSource, c: &CommandInvocation) -> Result<i32, Box<Text>> {
    // An argument the parser already resolved is always there; a failure without a message stands for the rest.
    let none = || Box::new(Text::literal(""));
    let Some(CommandArgumentValue::Players(targets)) = c.argument("targets") else { return Err(none()) };
    let targets = targets.clone();
    let model_id = match c.argument("model") {
        Some(CommandArgumentValue::String(s)) => s.clone(),
        Some(CommandArgumentValue::Id(id)) => id.to_string(),
        _ => return Err(none()),
    };
    let tier = match c.argument("tier") {
        Some(CommandArgumentValue::Custom { value, .. }) | Some(CommandArgumentValue::String(value)) => value.clone(),
        _ => return Err(none()),
    };
    // IntegerArgumentType.integer(0): the usage's `integer` has no bounds, so the floor is checked here, with
    // Brigadier's own message.
    let data = match c.argument("data") {
        Some(CommandArgumentValue::Integer(n)) => *n,
        _ => 0,
    };
    if data < 0 {
        // A Brigadier syntax error: the command fails with the message (failure text is red).
        return Err(Box::new(red(Text::translate(
            "argument.integer.low",
            vec![Text::literal("0"), Text::literal(data.to_string())],
        ))));
    }
    let (stack, display) = {
        let h: &dyn Host = &*host;
        let Some(model) = models::model(h, &model_id) else {
            let _ = host.commands().feedback(red(Text::literal(format!("Invalid model: {model_id}"))));
            return Ok(0);
        };
        // ModelTierRegistry.getValue(HostileNetworks.loc(tier.toLowerCase(Locale.ROOT))): null, then
        // getRequiredData(null) throws, for a tier that isn't there.
        let lower = tier.to_lowercase();
        let Some(t) = TIERS.iter().position(|d| d.name == lower) else {
            let _ = host.commands().feedback(red(Text::literal(format!("Invalid model tier: {tier}"))));
            return Ok(-1);
        };
        let Some(mut stack) = models::data_model(h, model) else { return Err(none()) };
        models::set_data(&mut stack, model.required(t).wrapping_add(data));
        // ItemStack.getDisplayName: the hover name in brackets.
        let name = h.hover_name(&stack).unwrap_or_else(|_| Text::literal(model_id.clone()));
        (stack, Text::literal("[").append(name).append(Text::literal("]")))
    };
    // Inventory.add, else a drop only that player may pick up at once; a data model stacks to 1, so each target
    // gets one stack (`player_give_stack` is `/give`'s own path, pickup sound included).
    for player in &targets {
        let _ = player.give_stack(host, stack.clone());
    }
    // Upstream sends `commands.give.success.single` in both branches; with several targets its third argument is
    // their count.
    let to = if targets.len() == 1 {
        Text::literal(targets[0].name(host).map(|n| n.to_string()).unwrap_or_default())
    } else {
        Text::literal(targets.len().to_string())
    };
    let _ = host.commands().feedback(Text::translate("commands.give.success.single", vec![Text::literal("1"), display, to]));
    Ok(targets.len() as i32)
}
