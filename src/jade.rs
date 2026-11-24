//! HostileJadePlugin through the Jade fork's provider API (`jade::`, ../jade-fork): the IO port's mode, under the
//! option "Data Center IO Port Mode". The machines' energy and slots need no provider: Jade reads every block's
//! ENERGY and ITEMS capabilities.

#[cfg(feature = "jade")]
use mod_api::*;

#[cfg(feature = "jade")]
const IO_PORT_MODE: &str = "hostilenetworks.io_port_mode";

#[cfg(feature = "jade")]
pub(crate) fn register() {
    jade::register_option(IO_PORT_MODE, "config.jade.plugin_hostilenetworks.io_port_mode", true);
    jade::register_client(lines);
}

#[cfg(feature = "jade")]
fn lines(host: &dyn ClientHost, target: &jade::Target, out: &mut Vec<jade::Line>) {
    let jade::Target::Block { block, state, .. } = target else { return };
    if block != crate::center::PORT || !jade::plugin_on(host, IO_PORT_MODE) {
        return;
    }
    let mode = state.split("mode=").nth(1).map(|m| m.trim_end_matches(']').split(',').next().unwrap_or("energy")).unwrap_or("energy");
    let word = jade::tr(&format!("hostilenetworks.io_port.mode.{mode}"), mode, &[]);
    out.push(jade::Line::text(jade::tr("hostilenetworks.io_port.mode_set", "Mode: %s", &[&word])));
}

#[cfg(not(feature = "jade"))]
pub(crate) fn register() {}
