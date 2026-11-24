//! Item names and hover text (appendHoverText) of every item: DataModelItem, MobPredictionItem, BlankDataModelItem,
//! FabDirectiveItem, TooltipBlockItem and the learner (its models are an image, `learner::tooltip_draw`).

use crate::center::model_item_name;
use crate::config;
use crate::fab;
use crate::models::{self, Instance};
use crate::gui::tip;
use crate::models_data::TIERS;
use crate::util::*;
use mod_api::*;

/// The stack's name: a data model's or prediction's "%s Data Model" / "%s Prediction" with its mob, else its item's.
pub(crate) fn name(host: &dyn ClientHost, stack: &ItemStack) -> String {
    let id = stack.item().id(host).unwrap_or_default();
    for kind in ["data_model", "prediction"] {
        if id == format!("hostilenetworks:{kind}") {
            return match models::stored(host, stack) {
                Some(m) => model_item_name(host, kind, m),
                None => lang_args(host, &format!("item.hostilenetworks.{kind}"), "%s", &["§kBROKEN§r".to_owned()]),
            };
        }
    }
    item_name(host, &id)
}

/// The item's own lines (appendHoverText), with § codes.
pub(crate) fn append(host: &dyn ClientHost, stack: &ItemStack, lines: &mut Vec<String>) {
    let id = stack.item().id(host).unwrap_or_default();
    let cfg = config::get(host);
    match id {
        "hostilenetworks:data_model" => {
            let Some(inst) = Instance::of(host, stack) else {
                lines.push(format!("{}§7§kBroke_AF", lang(host, "Error: %s", "Error: %s").replace("%s", "")));
                return;
            };
            let m = inst.model;
            let tier = format!("{}{}", code_for(TIERS[inst.tier].color), models::tier_name(host, inst.tier));
            lines.push(lang_args(host, "hostilenetworks.info.tier", "Tier: %s", &[format!("{tier}§r")]));
            if !inst.is_max() {
                let prog = lang_args(host, "hostilenetworks.info.dprog", "%s/%s", &[(inst.data - inst.tier_data()).to_string(), (inst.next_tier_data() - inst.tier_data()).to_string()]);
                lines.push(lang_args(host, "hostilenetworks.info.data", "Data Collected: %s", &[format!("§7{prog}§r")]));
                let key = if m.def.is_block { "hostilenetworks.info.dpm" } else { "hostilenetworks.info.dpk" };
                let gained = inst.data_gained(host);
                if gained == 0 {
                    let disabled = lang(host, "hostilenetworks.info.disabled", "(Disabled)");
                    lines.push(format!("{}§c{disabled}", lang_args(host, key, "%s", &["§7§k000 §r".to_owned()])));
                } else {
                    lines.push(lang_args(host, key, "%s", &[format!("§7{gained}§r")]));
                }
            }
            let rft = lang_args(host, "hostilenetworks.info.rft", "%s FE/t", &[m.sim_cost().to_string()]);
            lines.push(lang_args(host, "hostilenetworks.info.sim_cost", "Simulation Cost: %s", &[format!("§7{rft}§r")]));
            let variants = models::variant_names(host, m);
            if !variants.is_empty() {
                lines.push(lang(host, "hostilenetworks.info.subtypes", "Variants:"));
                for v in variants {
                    lines.push(tip(lang_args(host, "hostilenetworks.info.sub_list", "  - %s", &[v]), 0xFF00_FFC0));
                }
            }
        }
        "hostilenetworks:blank_data_model" => {
            if cfg.right_click_to_attune {
                let a = format!("§f{}§7", lang(host, "hostilenetworks.color_text.rclick", "Right-Click"));
                let b = format!("§6{}§7", lang(host, "hostilenetworks.color_text.build", "Build a Data Model"));
                lines.push(format!("§7{}", lang_args(host, "hostilenetworks.info.click_to_attune", "%s on an entity or block to %s", &[a, b])));
            } else {
                lines.push(format!("§7{}", lang(host, "hostilenetworks.info.attunment_disabled", "Model attunement is disabled")));
            }
        }
        "hostilenetworks:fab_directive" => {
            lines.push(format!("§7{}", lang(host, "item.hostilenetworks.fab_directive.desc", "")));
            lines.push(format!("§7{}", lang(host, "item.hostilenetworks.fab_directive.desc2", "")));
            directive_lines(host, stack, lines);
        }
        "hostilenetworks:sim_chamber" | "hostilenetworks:loot_fabricator" | "hostilenetworks:data_center" | "hostilenetworks:data_center_io_port" => {
            let n = if id == "hostilenetworks:sim_chamber" { 1 } else { 2 };
            let path = id.trim_start_matches("hostilenetworks:");
            for i in 0..n {
                let key = format!("block.hostilenetworks.{path}.desc{}", if i > 0 { (i + 1).to_string() } else { String::new() });
                lines.push(format!("§7{}", lang(host, &key, "")));
            }
        }
        _ => {}
    }
}

/// FabDirectiveItem.ClientAccess.appendHoverText: one stored selection at a time (the first; Placebo's tooltip
/// scrolling isn't on the fork).
fn directive_lines(host: &dyn ClientHost, stack: &ItemStack, lines: &mut Vec<String>) {
    let Some(Tag::Compound(saved)) = models::component(stack, models::FAB_SELECTIONS) else { return };
    let sels: Vec<(String, fab::Selection)> = match get_c(&saved, "selections") {
        Some(c) => c.iter().map(|(k, v)| (k.to_string_lossy(), fab::Selection::from_tag(v))).collect(),
        None => Vec::new(),
    };
    if sels.is_empty() {
        return;
    }
    lines.push(String::new());
    let shift = host.is_key_down("key.keyboard.left.shift") || host.is_key_down("key.keyboard.right.shift");
    let hint = if shift { lang(host, "text.hostilenetworks.scroll", "[scroll]") } else { lang(host, "text.hostilenetworks.shift", "[shift]") };
    let stored = lang_args(host, "text.hostilenetworks.stored_selection", "Stored Selection %s/%s", &["1".to_owned(), sels.len().to_string()]);
    lines.push(tip(format!("{stored} §8{hint}"), 0xFF00_FFC0));
    let (id, sel) = &sels[0];
    match models::model(host, id) {
        Some(m) => {
            let index = sel.current();
            match m.fab.get(index.max(0) as usize).filter(|_| index >= 0) {
                Some((drop, n)) => {
                    let input = model_item_name(host, "prediction", m);
                    let output = item_name(host, drop);
                    let queued = if sel.queue { sel.entries.len() as i32 - 1 } else { 0 };
                    let line = if queued > 0 {
                        lang_args(host, "text.hostilenetworks.selection.queue", "%s -> %s (+%s queued)", &[input, output, queued.to_string()])
                    } else {
                        lang_args(host, "text.hostilenetworks.selection", "%s -> %s %s", &[input, n.to_string(), output])
                    };
                    lines.push(tip(line, 0xFF00_FFC0));
                }
                None => lines.push(format!("§c{}", lang_args(host, "text.hostilenetworks.invalid_selection.index", "", &[index.to_string()]))),
            }
        }
        None => lines.push(format!("§c{}", lang_args(host, "text.hostilenetworks.invalid_selection.model", "", &[id.clone()]))),
    }
}

/// The whole tooltip of a stack as the screens that draw one themselves show it: its name, then its lines.
pub(crate) fn lines(host: &dyn ClientHost, stack: &ItemStack) -> Vec<String> {
    let mut out = vec![name(host, stack)];
    append(host, stack, &mut out);
    out
}
