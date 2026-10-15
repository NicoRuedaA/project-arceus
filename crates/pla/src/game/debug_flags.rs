//! Developer-only switches for the port's placeholder visuals (env vars, OFF by default).
//!
//! The real game shows none of these; they are stand-ins for assets the port does not have yet:
//! the generic proxy meshes of the field pickups (cubes, cones and emissive spheres for the 722
//! spawns) and of the world-event markers (50 m dome, 40 m outbreak pillar, loot sphere), and the
//! battle move-detail line the original battle panel used to print in port words next to the
//! game's own Power/Accuracy row ("Electric PP 10/10 | Pow 80 A60 S100"). Read as raw debug
//! clutter over the original HUD in the captures; the maintainer approved hiding them by default.
//!
//! Enable them for diagnostics with `PLA_DEBUG_PLACEHOLDERS=1` (`0` or unset keeps them off).
//! Gameplay is unaffected either way: pickups still stream, react and collect, the world-event
//! state machine still runs, and the battle keeps its data; only the visuals are opt-in.

/// `PLA_DEBUG_PLACEHOLDERS`: draw the generic pickup/event proxy meshes and the port-words battle
/// move-detail line. Off unless the variable is set to anything other than `0`.
pub fn placeholders() -> bool {
    enabled_from(std::env::var_os("PLA_DEBUG_PLACEHOLDERS"))
}

/// Pure part of [`placeholders`] (the env is process-global; tests check this directly).
fn enabled_from(value: Option<std::ffi::OsString>) -> bool {
    value.is_some_and(|v| v != "0")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn placeholders_are_off_unless_enabled() {
        assert!(!enabled_from(None), "unset = off");
        assert!(!enabled_from(Some(OsString::from("0"))), "0 = off");
        assert!(enabled_from(Some(OsString::from("1"))));
        assert!(enabled_from(Some(OsString::from("yes"))));
    }
}
