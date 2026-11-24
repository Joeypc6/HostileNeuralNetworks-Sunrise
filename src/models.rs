//! Data models and their tiers: `DataModel`, `EntityDataModel`, `BlockDataModel`, `ModelTier`, `ModelTierRegistry`,
//! `DataModelRegistry` and `DataModelInstance`, plus the item components that carry them (`Hostile.Components`).
//!
//! The registries are the jar's files, compiled in (`models_data.rs`); a model is loaded as the Java registry loads it:
//! its `neoforge:mod_loaded` conditions (that mod's on), the block models' `block_data_models_enabled` condition (the
//! config), unknown variants left out (`OPTIONAL_ENTITY_TYPE_LIST`) and optional drops of unknown items dropped.

use crate::config;
use crate::models_data::{MODELS, TIERS};
use crate::util::*;
use mod_api::*;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One model file of the jar.
pub(crate) struct ModelDef {
    pub id: &'static str,
    pub is_block: bool,
    /// `entity`, or `block`.
    pub source: &'static str,
    pub variants: &'static [&'static str],
    /// A `name` component as JSON, when the file gives one (else the entity's or block's name).
    pub name: Option<&'static str>,
    pub name_color: u32,
    pub scale: f32,
    pub offset: [f32; 3],
    pub sim_cost: i32,
    /// An item id, or `#tag`.
    pub input: &'static str,
    pub base_drop: (&'static str, u32),
    pub trivia: &'static str,
    /// (item, count, optional)
    pub fab: &'static [(&'static str, u32, bool)],
    /// RequiredData and DataGained overrides by tier, in TIER_ORDER.
    pub required: [Option<i32>; 5],
    pub gained: [Option<i32>; 5],
    pub needs: &'static [&'static str],
    pub block_condition: bool,
    pub not_silk_touch: bool,
}

/// One tier file of the jar.
pub(crate) struct TierDef {
    pub name: &'static str,
    pub required: i32,
    pub gained: i32,
    pub color: u32,
    pub accuracy: f32,
    pub can_sim: bool,
}

pub(crate) const TIER_ORDER: [&str; 5] = ["faulty", "basic", "advanced", "superior", "self_aware"];

/// A model as loaded: its file, the variants and drops this game has.
pub(crate) struct Model {
    pub def: &'static ModelDef,
    pub variants: Vec<&'static str>,
    /// Templates.create(fabDrops): (item id, count).
    pub fab: Vec<(&'static str, u32)>,
}

impl Model {
    pub fn id(&self) -> &'static str {
        self.def.id
    }
    pub fn sim_cost(&self) -> i32 {
        self.def.sim_cost
    }
    /// DataModel.getRequiredData: the model's override, else the tier's.
    pub fn required(&self, tier: usize) -> i32 {
        self.def.required[tier_slot(tier)].unwrap_or(TIERS[tier].required)
    }
    /// DataModel.getDataGained.
    pub fn gained(&self, tier: usize) -> i32 {
        self.def.gained[tier_slot(tier)].unwrap_or(TIERS[tier].gained)
    }
    /// Whether `kind` (an entity type id, or a block id for a block model) is this model's or one of its variants'.
    pub fn matches(&self, kind: &str) -> bool {
        self.def.source == kind || self.variants.iter().any(|v| *v == kind)
    }
    pub fn input_matches(&self, host: &dyn Host, stack: &ItemStack) -> bool {
        match self.def.input.strip_prefix('#') {
            Some(tag) => Identifier::parse(tag).ok().and_then(|t| host.tags().key(&t)).and_then(|k| host.tags().is_in_tag(stack.item(), k)).unwrap_or(false),
            None => is(host, stack, self.def.input),
        }
    }
    /// DataModel.input's first item, for the "Missing input" message.
    pub fn input_item(&self) -> &'static str {
        self.def.input.trim_start_matches('#')
    }
}

fn tier_slot(tier: usize) -> usize {
    TIER_ORDER.iter().position(|t| *t == TIERS[tier].name).unwrap_or(tier)
}

pub(crate) struct Table {
    pub models: BTreeMap<&'static str, Model>,
}

/// DataModelRegistry, loaded once a world's items and entity types are known.
pub(crate) fn table(host: &dyn Host) -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let entities: std::collections::HashSet<String> = host.entity_type_keys().into_iter().filter_map(|k| k.id(host)).map(|id| id.to_string()).collect();
        let blocks_on = config::get(host).enable_block_models;
        let mut models = BTreeMap::new();
        for def in MODELS {
            if def.needs.iter().any(|m| !host.on(m)) || (def.block_condition && !blocks_on) {
                continue;
            }
            let known = |id: &str| if def.is_block { host.block_state_id(id).is_some() } else { entities.contains(id) };
            if !known(def.source) {
                continue;
            }
            let variants = def.variants.iter().copied().filter(|v| known(v)).collect();
            let fab = def.fab.iter().filter(|(id, _, _)| item(host, id).is_some()).map(|(id, n, _)| (*id, *n)).collect();
            models.insert(def.id, Model { def, variants, fab });
        }
        Table { models }
    })
}

pub(crate) fn model(host: &dyn Host, id: &str) -> Option<&'static Model> {
    table(host).models.get(id)
}

// ---- The item handlers' filters without a host (`can_place_item`)

/// What the machines' `isValid` reads, by item key: the data model and prediction items, and each loaded model's
/// input items (its `input` ingredient: an item, or a tag's items). `can_place_item` has no host to read tags with, so
/// this is made from one: at server start, and again after tags change (by the next machine tick).
pub(crate) struct Filters {
    pub data_model: Option<ItemKey>,
    pub prediction: Option<ItemKey>,
    pub inputs: std::collections::HashMap<&'static str, std::collections::HashSet<ItemKey>>,
}

static FILTERS: std::sync::RwLock<Option<std::sync::Arc<Filters>>> = std::sync::RwLock::new(None);

pub(crate) fn refresh_filters(host: &dyn Host) {
    let keys = host.item_keys();
    let mut inputs = std::collections::HashMap::new();
    for m in table(host).models.values() {
        let set: std::collections::HashSet<ItemKey> = match m.def.input.strip_prefix('#') {
            Some(tag) => match Identifier::parse(tag).ok().and_then(|t| host.tags().key(&t)) {
                Some(k) => keys.iter().copied().filter(|i| host.tags().is_in_tag(*i, k) == Some(true)).collect(),
                None => Default::default(),
            },
            None => item(host, m.def.input).into_iter().collect(),
        };
        inputs.insert(m.id(), set);
    }
    let made = Filters { data_model: item(host, "hostilenetworks:data_model"), prediction: item(host, "hostilenetworks:prediction"), inputs };
    *FILTERS.write().unwrap_or_else(|e| e.into_inner()) = Some(std::sync::Arc::new(made));
}

/// Tags changed: the inputs are read again at the next chance with a host.
pub(crate) fn forget_filters() {
    *FILTERS.write().unwrap_or_else(|e| e.into_inner()) = None;
}

pub(crate) fn filters() -> Option<std::sync::Arc<Filters>> {
    FILTERS.read().unwrap_or_else(|e| e.into_inner()).clone()
}

impl Filters {
    /// DataModelItem.matchesModelInput: `model` is a data model whose stored model is loaded and takes `stack`.
    pub fn matches_input(&self, model: &ItemStack, stack: &ItemStack) -> bool {
        Some(model.item()) == self.data_model
            && stored_id(model).and_then(|id| self.inputs.get(id.as_str())).is_some_and(|set| set.contains(&stack.item()))
    }
}

// ---- Tiers (ModelTierRegistry)

pub(crate) fn max_tier() -> usize {
    TIERS.len() - 1
}

/// ModelTierRegistry.getByData: the highest tier whose required data (the model's) `data` reaches.
pub(crate) fn tier_of(m: &Model, data: i32) -> usize {
    (0..TIERS.len()).rev().find(|t| data >= m.required(*t)).unwrap_or(0)
}

pub(crate) fn next_tier(tier: usize) -> usize {
    (tier + 1).min(max_tier())
}

pub(crate) fn tier_key(tier: usize) -> String {
    format!("hostilenetworks.tier.{}", TIERS[tier].name)
}

// ---- Item components (Hostile.Components)

pub(crate) const DATA_MODEL: &str = "hostilenetworks:data_model";
pub(crate) const DATA: &str = "hostilenetworks:data";
pub(crate) const ITERATIONS: &str = "hostilenetworks:iterations";
pub(crate) const LEARNER_INV: &str = "hostilenetworks:learner_inv";
pub(crate) const FAB_SELECTIONS: &str = "hostilenetworks:fab_selections";
pub(crate) const COMPONENT_TYPES: [&str; 5] = [DATA_MODEL, DATA, ITERATIONS, LEARNER_INV, FAB_SELECTIONS];

/// A component of ours, by id, as its NBT tag.
pub(crate) struct Raw(pub &'static str);
impl Component for Raw {
    type Value = Tag;
    fn id(&self) -> &str {
        self.0
    }
    fn decode(&self, value: &Tag) -> Option<Tag> {
        Some(value.clone())
    }
    fn encode(&self, value: Tag) -> Tag {
        value
    }
}

pub(crate) fn component(stack: &ItemStack, id: &'static str) -> Option<Tag> {
    stack.get(&Raw(id))
}

pub(crate) fn set_component(stack: &mut ItemStack, id: &'static str, value: Tag) {
    let _ = stack.set(&Raw(id), value);
}

/// DataModelItem.getStoredModel's id.
pub(crate) fn stored_id(stack: &ItemStack) -> Option<String> {
    match component(stack, DATA_MODEL)? {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

pub(crate) fn int_component(stack: &ItemStack, id: &'static str) -> i32 {
    match component(stack, id) {
        Some(Tag::Int(v)) => v,
        _ => 0,
    }
}

pub(crate) fn data_of(stack: &ItemStack) -> i32 {
    int_component(stack, DATA)
}

pub(crate) fn iters_of(stack: &ItemStack) -> i32 {
    int_component(stack, ITERATIONS)
}

pub(crate) fn set_data(stack: &mut ItemStack, v: i32) {
    set_component(stack, DATA, Tag::Int(v));
}

pub(crate) fn set_iters(stack: &mut ItemStack, v: i32) {
    set_component(stack, ITERATIONS, Tag::Int(v));
}

/// The model `stack` holds, if it's bound (a data model or a prediction).
pub(crate) fn stored(host: &dyn Host, stack: &ItemStack) -> Option<&'static Model> {
    model(host, &stored_id(stack)?)
}

/// `new ItemStack(item)` with DataModelItem.setStoredModel.
pub(crate) fn model_stack(host: &dyn Host, item_id: &str, m: &Model, count: u32) -> Option<ItemStack> {
    let mut s = stack_of(host, item_id, count)?;
    set_component(&mut s, DATA_MODEL, Tag::String(m.id().into()));
    set_component(&mut s, "minecraft:item_name", item_name_tag(item_id, m));
    Some(s)
}

/// DataModelItem.getName / MobPredictionItem.getName as the stack's `minecraft:item_name`: the item's "%s Data
/// Model" / "%s Prediction" with the model's name (`name()`'s plain copy), translated on each client, so the stack
/// is named for its mob everywhere (the hotbar, chat, a recipe viewer), not only in its tooltip.
pub(crate) fn item_name_tag(item_id: &str, m: &Model) -> Tag {
    let mut arg = Compound::new();
    let translate = m.def.name.and_then(|j| json_key(j, "translate"));
    let text = m.def.name.and_then(|j| json_key(j, "text"));
    if let Some(key) = translate {
        arg.insert(java("translate"), Tag::String(key.as_str().into()));
    } else if let Some(t) = text {
        arg.insert(java("text"), Tag::String(t.as_str().into()));
    } else {
        let (ns, path) = m.def.source.split_once(':').unwrap_or(("minecraft", m.def.source));
        let kind = if m.def.is_block { "block" } else { "entity" };
        arg.insert(java("translate"), Tag::String(format!("{kind}.{ns}.{path}").as_str().into()));
    }
    let mut c = Compound::new();
    c.insert(java("translate"), Tag::String(format!("item.{}", item_id.replace(':', ".")).as_str().into()));
    c.insert(java("with"), Tag::List(vec![Tag::Compound(arg)]));
    Tag::Compound(c)
}

fn json_key(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let i = json.find(&needle)?;
    let rest = &json[i + needle.len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(rest.chars().take_while(|c| *c != '"').collect())
}

/// DataModel.getPredictionDrop.
pub(crate) fn prediction(host: &dyn Host, m: &Model, count: u32) -> Option<ItemStack> {
    model_stack(host, "hostilenetworks:prediction", m, count)
}

pub(crate) fn data_model(host: &dyn Host, m: &Model) -> Option<ItemStack> {
    model_stack(host, "hostilenetworks:data_model", m, 1)
}

// ---- DataModelInstance

/// A data model stack's model, data and tier.
pub(crate) struct Instance {
    pub model: &'static Model,
    pub data: i32,
    pub tier: usize,
}

impl Instance {
    pub fn of(host: &dyn Host, stack: &ItemStack) -> Option<Self> {
        if !is(host, stack, "hostilenetworks:data_model") {
            return None;
        }
        let model = stored(host, stack)?;
        let data = data_of(stack);
        Some(Self { model, data, tier: tier_of(model, data) })
    }
    pub fn is_max(&self) -> bool {
        self.tier == max_tier()
    }
    pub fn tier_data(&self) -> i32 {
        self.model.required(self.tier)
    }
    pub fn next_tier_data(&self) -> i32 {
        self.model.required(next_tier(self.tier))
    }
    /// getDataGained: none while world actions don't upgrade models.
    pub fn data_gained(&self, host: &dyn Host) -> i32 {
        if config::get(host).action_upgrades_model { self.model.gained(self.tier) } else { 0 }
    }
    /// getAccuracy.
    pub fn accuracy(&self, host: &dyn Host) -> f32 {
        let tier = &TIERS[self.tier];
        if config::get(host).continuous_accuracy && !self.is_max() {
            let next = &TIERS[next_tier(self.tier)];
            let diff = self.next_tier_data() - self.tier_data();
            let t_diff = next.accuracy - tier.accuracy;
            tier.accuracy + t_diff * (diff - (self.next_tier_data() - self.data)) as f32 / diff as f32
        } else {
            tier.accuracy
        }
    }
    /// rollPredictions.
    pub fn roll(&self, host: &dyn Host, salt: Pos) -> i32 {
        let accuracy = self.accuracy(host);
        let floor = accuracy as i32;
        floor + i32::from(next_float(salt) < accuracy - floor as f32)
    }
    /// getActionsNeeded: Mth.ceil((next - data) / gained).
    pub fn actions_needed(&self, host: &dyn Host) -> i32 {
        let gained = self.data_gained(host);
        if gained == 0 {
            return i32::MAX;
        }
        ((self.next_tier_data() - self.data) as f32 / gained as f32).ceil() as i32
    }
}

// ---- Names (client)

/// DataModel.name(): the file's name, else the mob's or block's, in the model's colour.
pub(crate) fn name(host: &dyn ClientHost, m: &Model) -> String {
    if let Some(json) = m.def.name {
        return json_text(host, json);
    }
    if m.def.is_block { block_name(host, m.def.source) } else { entity_name(host, m.def.source) }
}

/// A text component's plain words from its JSON (`translate` looked up, `text` as is).
fn json_text(host: &dyn ClientHost, json: &str) -> String {
    let inner = json.trim().trim_matches('"');
    if let Some(i) = json.find("\"translate\"") {
        let rest = &json[i + 11..];
        if let Some(start) = rest.find('"') {
            let key: String = rest[start + 1..].chars().take_while(|c| *c != '"').collect();
            return host.translate(&key).unwrap_or(key);
        }
    }
    if let Some(i) = json.find("\"text\"") {
        let rest = &json[i + 6..];
        if let Some(start) = rest.find('"') {
            return rest[start + 1..].chars().take_while(|c| *c != '"').collect();
        }
    }
    inner.to_owned()
}

/// The variant names (`variantNames`).
pub(crate) fn variant_names(host: &dyn ClientHost, m: &Model) -> Vec<String> {
    m.variants.iter().map(|v| if m.def.is_block { block_name(host, v) } else { entity_name(host, v) }).collect()
}

/// ModelTier.getComponent: the tier's name in its colour.
pub(crate) fn tier_name(host: &dyn ClientHost, tier: usize) -> String {
    lang(host, &tier_key(tier), TIERS[tier].name)
}
