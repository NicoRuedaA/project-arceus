//! First sheet-driven subsystem: registries into the ECS, then one dispatch
//! pass per tick (The Spreadsheet Method, sections 5.7 and 9.1).

use bevy::prelude::*;

use crate::{dispatch, generated};

/// Shared port context; the generated sheets are the data, this is the runtime
/// state the dispatch table mutates.
#[derive(Resource, Default, Debug)]
pub struct PortContext {
    pub subsystems_loaded: usize,
    pub registry_hits: usize,
    pub dispatched: usize,
    /// Subsystems whose strategy is `port`.
    pub port_targets: usize,
    /// Subsystems whose strategy is `stub`/`replace`/`skip`.
    pub delegated: usize,
}

/// Build the headless port app: load registries at startup, dispatch every tick.
pub fn build_app() -> App {
    let mut app = App::new();
    app.insert_resource(PortContext::default());
    app.add_systems(Startup, load_registries);
    app.add_systems(Update, run_dispatch);
    app
}

fn load_registries(mut ctx: ResMut<PortContext>) {
    ctx.subsystems_loaded = generated::domain_subsystems::COUNT;
    // Cold-path lookups through the generated PHF registry, and a consistency
    // check that the dense index and the registry agree row by row.
    for index in generated::domain_subsystems::ALL {
        if generated::domain_subsystems::REGISTRY
            .get(index.row().id)
            .is_some()
        {
            ctx.registry_hits += 1;
        }
    }
    // Strategy sheet decides what the port owns versus what it delegates.
    for row in generated::domain_port_strategy::ROWS {
        if row.decision == "port" {
            ctx.port_targets += 1;
        } else {
            ctx.delegated += 1;
        }
    }
}

fn run_dispatch(mut ctx: ResMut<PortContext>) {
    for index in generated::domain_subsystems::ALL {
        dispatch::BEHAVIORS[index.to_index() as usize](&mut ctx, *index);
    }
    ctx.dispatched += 1;
}
