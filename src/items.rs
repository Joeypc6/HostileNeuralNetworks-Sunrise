//! The machines' item and energy capabilities, as the Java tiles expose them to automation:
//! - SimItemHandler: inserts into 0 (a data model) and 1 (the model's input), extracts from 2 and 3;
//! - FabItemHandler: inserts into 0 (a prediction), extracts from 1-16;
//! - DataCenterItemHandler: inserts into 0-28 (models, then inputs any installed model takes), extracts from 25-44;
//! - the IO port's proxies (ModelPortHandler, OffsetRangedPortHandler, the controller's energy) by its mode, while its
//!   controller's shell is whole.
//!
//! A handler holds no state: it reads and writes the block entity's slots through the host, in the transaction it is
//! given, so a move that aborts leaves nothing changed.

use crate::center;
use crate::fab;
use crate::sim;
use crate::util::*;
use mod_api::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Sim,
    Fab,
    Center,
}

impl Kind {
    fn size(self) -> usize {
        match self {
            Kind::Sim => 4,
            Kind::Fab => 17,
            Kind::Center => 45,
        }
    }
    fn may_insert(self, slot: usize) -> bool {
        match self {
            Kind::Sim => slot <= 1,
            Kind::Fab => slot == 0,
            Kind::Center => slot < 29,
        }
    }
    fn may_extract(self, slot: usize) -> bool {
        match self {
            Kind::Sim => slot > 1,
            Kind::Fab => slot != 0,
            Kind::Center => slot >= 25,
        }
    }
    fn valid(self, host: &dyn Host, items: &[Option<ItemStack>], slot: usize, stack: &ItemStack) -> bool {
        match self {
            Kind::Sim => sim::valid(host, items, slot, stack),
            Kind::Fab => slot != 0 || is(host, stack, "hostilenetworks:prediction"),
            Kind::Center => center::valid(host, items, slot, stack),
        }
    }
}

/// The slots of the block entity at `at` as one handler, offset and limited (a port's view of a range), with or
/// without the tile's own insert and extract limits.
pub(crate) struct Slots {
    pub level: LevelRef,
    pub pos: Pos,
    pub kind: Kind,
    pub offset: usize,
    pub count: usize,
    /// ModelPortHandler extracts with extractInternal: the models come out through a port.
    pub internal_extract: bool,
}

impl Slots {
    pub fn whole(at: BlockSide, kind: Kind) -> Self {
        Self { level: at.level, pos: at.pos, kind, offset: 0, count: kind.size(), internal_extract: false }
    }
    fn all(&self, host: &dyn Host) -> Vec<Option<ItemStack>> {
        let Some(be) = host.block_entity(self.level, self.pos) else { return Vec::new() };
        (0..self.kind.size() as u32).map(|i| be.item_slot(host, i).ok().flatten()).collect()
    }
    fn get(&self, host: &dyn Host, index: usize) -> Option<ItemStack> {
        if index >= self.count {
            return None;
        }
        host.block_entity(self.level, self.pos)?.item_slot(host, (self.offset + index) as u32).ok().flatten()
    }
    fn put(&self, host: &mut dyn ServerHost, tx: &TransactionContext, index: usize, stack: Option<ItemStack>) -> bool {
        let Some(be) = host.block_entity(self.level, self.pos) else { return false };
        be.set_item_slot(host, tx, (self.offset + index) as u32, stack).unwrap_or(false)
    }
}

impl ResourceHandler<ItemResource> for Slots {
    fn size(&self, _host: &mut dyn ServerHost) -> usize {
        self.count
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.get(&*host, index).map(|s| ItemResource::of(&s))
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.get(&*host, index).map_or(0, |s| s.count())
    }
    fn capacity(&self, host: &mut dyn ServerHost, index: usize, resource: Option<&ItemResource>) -> u32 {
        if index >= self.count {
            return 0;
        }
        // The general limit: the handler's slot limit (`getSlotLimit`, 64).
        resource.map_or(64, |r| max_stack(&*host, r.as_stack()))
    }
    fn is_valid(&self, host: &mut dyn ServerHost, index: usize, resource: &ItemResource) -> bool {
        if index >= self.count {
            return false;
        }
        let items = self.all(&*host);
        self.kind.valid(&*host, &items, self.offset + index, resource.as_stack())
    }
    fn insert_at(&self, host: &mut dyn ServerHost, tx: &TransactionContext, index: usize, resource: &ItemResource, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let slot = self.offset + index;
        if amount == 0 || index >= self.count || !self.kind.may_insert(slot) {
            return Ok(0);
        }
        let items = self.all(&*host);
        if !self.kind.valid(&*host, &items, slot, resource.as_stack()) {
            return Ok(0);
        }
        let limit = max_stack(&*host, resource.as_stack());
        let have = items.get(slot).cloned().flatten();
        let room = match &have {
            None => limit,
            Some(h) if resource.matches(h) => limit.saturating_sub(h.count()),
            Some(_) => 0,
        };
        let n = room.min(amount);
        if n == 0 {
            return Ok(0);
        }
        let new = match have {
            Some(h) => with_count(&h, h.count() + n),
            None => resource.to_stack(n),
        };
        Ok(if self.put(host, tx, index, new) { n } else { 0 })
    }
    fn extract_at(&self, host: &mut dyn ServerHost, tx: &TransactionContext, index: usize, resource: &ItemResource, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        let slot = self.offset + index;
        if amount == 0 || index >= self.count || (!self.internal_extract && !self.kind.may_extract(slot)) {
            return Ok(0);
        }
        let Some(have) = self.get(&*host, index) else { return Ok(0) };
        if !resource.matches(&have) {
            return Ok(0);
        }
        let n = have.count().min(amount);
        let rest = have.shrink(n);
        Ok(if self.put(host, tx, index, rest) { n } else { 0 })
    }
}

/// The port's `mode` property, from its state.
fn port_mode(level: &dyn LevelHost, state: BlockState) -> Option<String> {
    let text = level.state_string(state)?;
    let (_, props) = text.split_once('[')?;
    props.trim_end_matches(']').split(',').find_map(|p| p.strip_prefix("mode=")).map(str::to_owned)
}

/// The IO port's item handler by its mode: none for energy, or without a controller.
pub(crate) fn port_items(
    level: &dyn LevelHost,
    at: BlockSide,
    state: BlockState,
    port: BlockEntityRef,
    offer: &mut BlockCapabilityOffer<'_, dyn ResourceHandler<ItemResource>>,
) {
    let Some(owner) = center::port_owner_in(level, port) else { return };
    let Some(mode) = port_mode(level, state) else { return };
    let (offset, count, internal_extract) = match mode.as_str() {
        "models" => (0, 25, true),
        "inputs" => (25, 4, false),
        "outputs" => (29, 16, false),
        _ => return,
    };
    offer.offer(|| Box::new(Slots { level: at.level, pos: owner, kind: Kind::Center, offset, count, internal_extract }));
}

/// The controller's energy, through a port in energy mode.
pub(crate) struct PortEnergy {
    level: LevelRef,
    owner: Pos,
}

impl EnergyHandler for PortEnergy {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        self.level.capability(host, &ENERGY, self.owner, None).map_or(0, |c| c.amount(host))
    }
    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        self.level.capability(host, &ENERGY, self.owner, None).map_or(0, |c| c.capacity(host))
    }
    fn insert(&self, host: &mut dyn ServerHost, tx: &TransactionContext, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        self.level.capability(host, &ENERGY, self.owner, None).map_or(Ok(0), |c| c.insert(host, tx, amount))
    }
    fn extract(&self, host: &mut dyn ServerHost, tx: &TransactionContext, amount: u32) -> Result<u32, TransactionError<Overanswered>> {
        self.level.capability(host, &ENERGY, self.owner, None).map_or(Ok(0), |c| c.extract(host, tx, amount))
    }
}

pub(crate) fn port_energy(
    level: &dyn LevelHost,
    at: BlockSide,
    state: BlockState,
    port: BlockEntityRef,
    offer: &mut BlockCapabilityOffer<'_, dyn EnergyHandler>,
) {
    if port_mode(level, state).as_deref() != Some("energy") {
        return;
    }
    let Some(owner) = center::port_owner_in(level, port) else { return };
    offer.offer(|| Box::new(PortEnergy { level: at.level, owner }));
}

/// The machine of `kind` at `at`, with the tile's own limits.
pub(crate) fn machine_items(at: BlockSide, kind: Kind) -> Box<dyn ResourceHandler<ItemResource>> {
    Box::new(Slots::whole(at, kind))
}

pub(crate) fn _unused(_: &fab::Selection) {}
