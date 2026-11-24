//! HostileConfig: `config/hostilenetworks.cfg` in Placebo's format, as the Java mod writes it. The server reads it
//! (again at each server start) and sends it to every client that joins (HostileConfig.ConfigPayload), so a client
//! shows the server's numbers. The HUD offset is the client's own.

use mod_api::*;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Config {
    pub sim_power_cap: i32,
    pub fab_power_cap: i32,
    pub fab_power_cost: i32,
    pub data_center_power_cap: i32,
    pub right_click_to_attune: bool,
    /// 0 disabled (no training mode), 1 yes, 2 only up to tier boundaries.
    pub sim_model_upgrade: i32,
    pub action_upgrades_model: bool,
    pub continuous_accuracy: bool,
    /// Not synced: which models load.
    pub enable_block_models: bool,
    pub data_center_sim_cost: f32,
    /// Client only: the Deep Learner HUD's anchor and offset (Placebo's Offset).
    pub hud_anchor: String,
    pub hud_x: i32,
    pub hud_y: i32,
}

impl Default for Config {
    fn default() -> Self {
        parse("")
    }
}

const FILE: &str = "hostilenetworks.cfg";

const DEFAULT: &str = "\
# Hostile Networks Config
# All entries in this config file are synced from server to client unless noted otherwise.

client {
    # Client-only options, not synced
    S:\"Deep Learner HUD\"=top_left,0,0
}

models {
    # Whether killing mobs and breaking blocks will upgrade the data on a corresponding model. Note: If you disable this, be sure to add a way for players to get non-Faulty models!
    B:\"Action Upgrades Model\"=true

    # If true, the accuracy of the model increases as it gains progress towards the next tier. If false, always uses the base accuracy of the current tier.
    B:\"Continuous Accuracy\"=true

    # Per-tick power cost the Data Center pays per active model, as a multiplier on the model's base simCost. 1.5 = 150% of normal.
    # Min: 0.01 Max: 100.0
    S:\"Data Center Sim Cost Multiplier\"=1.5

    # If true, block data models (such as the built-in ore models) are loaded. This is an experimental feature. This value is not synced; it governs which models load during datapack reading.
    B:\"Enable Block Data Models\"=false

    # If true, right clicking a blank data model on a mob or block will attune it to that target. If disabled, you will need to provide players with a way to get attuned models!
    B:\"Right Click To Attune\"=true

    # How the Simulation Chamber's Training Mode upgrades the data on a model. (0 = Disabled - also disables Training Mode entirely, 1 = Yes, 2 = Only up to tier boundaries)
    # Min: 0 Max: 2
    I:\"Sim Chamber Upgrades Model\"=1
}

power {
    # The maximum FE stored in the Data Center. Sized for 25 concurrent self-aware simulations.
    # Min: 1 Max: 2147483647
    I:\"Data Center Power Cap\"=8000000

    # The maximum FE stored in the Loot Fabricator.
    # Min: 1 Max: 2147483647
    I:\"Loot Fab Power Cap\"=1000000

    # The FE/t cost of the Loot Fabricator.
    # Min: 0 Max: 2147483647
    I:\"Loot Fab Power Cost\"=256

    # The maximum FE stored in the Simulation Chamber.
    # Min: 1 Max: 2147483647
    I:\"Sim Chamber Power Cap\"=2000000
}
";

static FILE_CONFIG: Mutex<Option<Config>> = Mutex::new(None);
/// What the server sent this client.
static SYNCED: Mutex<Option<Config>> = Mutex::new(None);

/// The options in force: the server's, as it sent them, on a client that has them; else the file.
pub(crate) fn get(host: &dyn Host) -> Config {
    if let Some(c) = SYNCED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        let mut c = c;
        let own = file(host);
        c.enable_block_models = own.enable_block_models;
        c.hud_anchor = own.hud_anchor;
        c.hud_x = own.hud_x;
        c.hud_y = own.hud_y;
        return c;
    }
    file(host)
}

fn file(host: &dyn Host) -> Config {
    let mut guard = FILE_CONFIG.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(|| parse(&read(host))).clone()
}

/// HostileEvents.reload: the file read again (a server start).
pub(crate) fn reload(host: &dyn Host) {
    let fresh = parse(&read(host));
    *FILE_CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(fresh);
}

/// The client left a world: its server's numbers go.
pub(crate) fn forget_synced() {
    *SYNCED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn read(host: &dyn Host) -> String {
    let files = host.config_files();
    let Ok(path) = ConfigPath::new(FILE) else { return DEFAULT.to_owned() };
    match files.read(&path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => {
            let _ = files.write(&path, DEFAULT.as_bytes());
            DEFAULT.to_owned()
        }
    }
}

/// Placebo's lines: `T:"Name"=value` (or `T:Name=value`) in `category { }` blocks.
pub(crate) fn parse(text: &str) -> Config {
    let read = |key: &str| {
        text.lines().find_map(|line| {
            let line = line.trim();
            let (k, v) = line.split_once('=')?;
            let k = k.split_once(':').map_or(k, |(_, n)| n).trim().trim_matches('"');
            (k == key).then(|| v.trim().to_owned())
        })
    };
    let int = |key: &str, d: i32, lo: i32, hi: i32| read(key).and_then(|v| v.parse::<i32>().ok()).unwrap_or(d).clamp(lo, hi);
    let flag = |key: &str, d: bool| read(key).and_then(|v| v.parse().ok()).unwrap_or(d);
    // The Java mod renamed "Killing Upgrades Model".
    let action = read("Action Upgrades Model").or_else(|| read("Killing Upgrades Model")).and_then(|v| v.parse().ok()).unwrap_or(true);
    let hud = read("Deep Learner HUD").unwrap_or_else(|| "top_left,0,0".to_owned());
    let mut hud_parts = hud.split(',');
    let anchor = hud_parts.next().unwrap_or("top_left").trim().to_owned();
    let hx = hud_parts.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    let hy = hud_parts.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
    Config {
        sim_power_cap: int("Sim Chamber Power Cap", 2_000_000, 1, i32::MAX),
        fab_power_cap: int("Loot Fab Power Cap", 1_000_000, 1, i32::MAX),
        fab_power_cost: int("Loot Fab Power Cost", 256, 0, i32::MAX),
        data_center_power_cap: int("Data Center Power Cap", 8_000_000, 1, i32::MAX),
        right_click_to_attune: flag("Right Click To Attune", true),
        sim_model_upgrade: int("Sim Chamber Upgrades Model", 1, 0, 2),
        action_upgrades_model: action,
        continuous_accuracy: flag("Continuous Accuracy", true),
        enable_block_models: flag("Enable Block Data Models", false),
        data_center_sim_cost: read("Data Center Sim Cost Multiplier").and_then(|v| v.parse().ok()).unwrap_or(1.5f32).clamp(0.01, 100.0),
        hud_anchor: anchor,
        hud_x: hx,
        hud_y: hy,
    }
}

// ---- ConfigPayload: server to client

pub(crate) const NET_CONFIG: &str = "hostilenetworks:config";

pub(crate) fn encode(c: &Config) -> Vec<u8> {
    format!(
        "{} {} {} {} {} {} {} {} {}",
        c.sim_power_cap,
        c.fab_power_cap,
        c.fab_power_cost,
        c.data_center_power_cap,
        c.right_click_to_attune,
        c.sim_model_upgrade,
        c.action_upgrades_model,
        c.continuous_accuracy,
        c.data_center_sim_cost
    )
    .into_bytes()
}

/// The client's copy of what the server sent; a malformed payload is ignored.
pub(crate) fn receive(data: &[u8]) {
    if data.len() > 256 {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else { return };
    let f: Vec<&str> = text.split(' ').collect();
    let [a, b, c, d, e, g, h, i, j] = f.as_slice() else { return };
    let parsed = (|| {
        Some(Config {
            sim_power_cap: a.parse().ok()?,
            fab_power_cap: b.parse().ok()?,
            fab_power_cost: c.parse().ok()?,
            data_center_power_cap: d.parse().ok()?,
            right_click_to_attune: e.parse().ok()?,
            sim_model_upgrade: g.parse().ok()?,
            action_upgrades_model: h.parse().ok()?,
            continuous_accuracy: i.parse().ok()?,
            data_center_sim_cost: j.parse().ok()?,
            ..Config::default()
        })
    })();
    if let Some(cfg) = parsed {
        *SYNCED.lock().unwrap_or_else(|e| e.into_inner()) = Some(cfg);
    }
}

/// The file's numbers as the game starts, for what registration fixes (the machines' energy buffers).
// SUNRISE-GAP: `block_energy` is declared once at registration, so the power caps come from the file as the game
// starts; the fork asked a hook at each lookup, and a cap edited between two world starts without a restart only took
// effect for the buffers then. Every other use of the caps still reads `get`.
pub(crate) fn at_startup(cx: &RegistrationContext) -> Config {
    let path = cx
        .instance_dir()
        .and_then(InstanceDirectory::to_path_buf)
        .map(|base| base.join("config").join(FILE));
    let text = path.and_then(|p| std::fs::read(p).ok()).map(|b| String::from_utf8_lossy(&b).into_owned());
    parse(text.as_deref().unwrap_or(DEFAULT))
}
