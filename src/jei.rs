//! HostileJeiPlugin through the JEI fork's API (`jei::api`, ../jei-fork): the Simulation Chamber category (one card
//! per data model: the model and its input to the base drop and the model's prediction), the Loot Fabricator
//! category (one card per model and fabricator drop: its prediction to the drop) and the Data Center's and IO port's
//! information pages. Registered on the client, only where JEI is installed.

#[cfg(feature = "jei")]
use crate::models;
#[cfg(feature = "jei")]
use mod_api::*;

#[cfg(feature = "jei")]
pub(crate) fn register() {
    use jei::api::*;
    register_category(CategoryDef::new("hostilenetworks:sim_chamber", "block.hostilenetworks.sim_chamber", "hostilenetworks:sim_chamber"));
    register_category(CategoryDef::new("hostilenetworks:loot_fabricator", "block.hostilenetworks.loot_fabricator", "hostilenetworks:loot_fabricator"));
    // HostileJeiPlugin.registerItemSubtypes: data models and predictions are told apart by their model.
    register_subtype("hostilenetworks:data_model", "hostilenetworks:data_model");
    register_subtype("hostilenetworks:prediction", "hostilenetworks:data_model");
    register_recipes("hostilenetworks", recipes);
    add_info(
        &["hostilenetworks:data_center"],
        &["hostilenetworks.jei.data_center.intro", "hostilenetworks.jei.data_center.shell", "hostilenetworks.jei.data_center.operation", "hostilenetworks.jei.data_center.io_port"],
    );
    add_info(&["hostilenetworks:data_center_io_port"], &["hostilenetworks.jei.io_port.intro", "hostilenetworks.jei.io_port.modes"]);
}

#[cfg(feature = "jei")]
fn recipes(host: &dyn Host) -> Vec<(String, jei::api::RecipeDef)> {
    use jei::api::RecipeDef;
    let mut out = Vec::new();
    let table = models::table(host);
    for m in table.models.values() {
        let key = m.id().replace(':', "/");
        // The model's own stacks, with their components (the JEI fork's item spec: the id, then the JSON).
        let spec = |item: &str| {
            let name = match models::item_name_tag(item, m) {
                Tag::Compound(c) => {
                    let arg = match c.get(&crate::util::java("with")) {
                        Some(Tag::List(l)) => match l.first() {
                            Some(Tag::Compound(a)) => a.iter().map(|(k, v)| format!("{:?}:{:?}", k.to_string_lossy(), if let Tag::String(s) = v { s.to_string_lossy() } else { String::new() })).collect::<Vec<_>>().join(","),
                            _ => String::new(),
                        },
                        _ => String::new(),
                    };
                    format!("{{\"translate\":{:?},\"with\":[{{{arg}}}]}}", format!("item.{}", item.replace(':', ".")))
                }
                _ => String::new(),
            };
            format!("{item}{{\"hostilenetworks:data_model\":{:?},\"minecraft:item_name\":{name}}}", m.id())
        };
        let model_spec = spec("hostilenetworks:data_model");
        let pred_spec = spec("hostilenetworks:prediction");
        // SimChamberCategory: 116 by 43; the model at 4,4, its input at 28,4, the base drop at 96,4, the prediction at
        // 66,26.
        out.push((
            "hostilenetworks:sim_chamber".to_owned(),
            RecipeDef::new(&format!("hostilenetworks:sim_chamber/{key}"), [116.0, 43.0])
                .background("hostilenetworks:gui/jei/sim_chamber", [0.0, 0.0, 116.0, 43.0])
                .input(4.0, 4.0, &[(model_spec.as_str(), 1)])
                .input(28.0, 4.0, &[(m.input_item(), 1)])
                .output(96.0, 4.0, &[(m.def.base_drop.0, m.def.base_drop.1)])
                .output(66.0, 26.0, &[(pred_spec.as_str(), 1)]),
        ));
        // LootFabCategory: 103 by 30; the prediction at 9,7, the drop at 79,7.
        for (i, (drop, n)) in m.fab.iter().enumerate() {
            out.push((
                "hostilenetworks:loot_fabricator".to_owned(),
                RecipeDef::new(&format!("hostilenetworks:loot_fabricator/{key}/{i}"), [103.0, 30.0])
                    .background("hostilenetworks:gui/jei/loot_fabricator", [0.0, 0.0, 103.0, 30.0])
                    .input(9.0, 7.0, &[(pred_spec.as_str(), 1)])
                    .output(79.0, 7.0, &[(*drop, *n)]),
            ));
        }
    }
    out
}

#[cfg(not(feature = "jei"))]
pub(crate) fn register() {}
