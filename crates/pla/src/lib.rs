//! Pokemon Legends: Arceus port crate.
//!
//! The sheet book is the source of truth; this crate consumes generated
//! modules. Nothing here is hand-edited generated data.

pub mod app;
pub mod assets;
pub mod dispatch;
pub mod event;
pub mod host;
pub mod render;
pub mod save;
pub mod script;

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/sheets/index.rs"));
}

#[cfg(test)]
mod tests {
    use super::generated;

    #[test]
    fn registry_is_populated() {
        assert!(
            generated::SHEET_COUNTS.len() >= 5,
            "expected at least five emitted sheets, got {}",
            generated::SHEET_COUNTS.len()
        );
    }

    #[test]
    fn plan_rows_match_count() {
        assert_eq!(generated::plan::COUNT, generated::plan::ROWS.len());
    }

    #[test]
    fn doctrine_has_project_row() {
        assert!(generated::doctrine::ROWS.iter().any(|r| r.key == "project"));
    }

    #[test]
    fn phf_registry_lookup() {
        // cold path: PHF map by id
        assert!(generated::plan::REGISTRY.get("port_target").is_some());
        assert!(generated::plan::REGISTRY.get("does_not_exist").is_none());
    }

    #[test]
    fn dense_index_covers_all_rows() {
        use generated::domain_subsystems::{ALL, COUNT, ROWS};
        assert_eq!(ALL.len(), COUNT);
        assert_eq!(ROWS.len(), COUNT);
        assert_eq!(ALL[0].to_index(), 0);
        assert_eq!(ALL[COUNT - 1].row().id, ROWS[COUNT - 1].id);
    }
}

#[cfg(test)]
mod app_tests {
    use super::app::{build_app, PortContext};
    use super::generated;

    #[test]
    fn app_loads_registries_and_dispatches() {
        let mut app = build_app();
        app.update(); // Startup + first Update
        let ctx = app.world().resource::<PortContext>();
        assert_eq!(ctx.subsystems_loaded, generated::domain_subsystems::COUNT);
        assert_eq!(ctx.registry_hits, generated::domain_subsystems::COUNT);
        assert_eq!(ctx.dispatched, 1);
        assert!(ctx.port_targets >= 8, "port targets: {}", ctx.port_targets);
        assert!(ctx.delegated >= 8, "delegated: {}", ctx.delegated);
    }
}
