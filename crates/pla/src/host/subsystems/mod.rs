//! Host subsystems: one file per engine subsystem.
//!
//! ## Adding a subsystem
//!
//! 1. Create `subsystems/<name>.rs`:
//!
//!    ```ignore
//!    use mlua::{Lua, Result};
//!    use crate::host::{stub_fallback, SaveHandle};
//!
//!    pub fn install(lua: &Lua, save: &SaveHandle) -> Result<()> {
//!        // real bindings here; stub_fallback() for what is not ported yet
//!    }
//!    ```
//!
//! 2. Add one line to [`SUBSYSTEMS`].
//!
//! The rest of the host (Lua prelude, `FnvHash64`, the stub fallback) is frozen
//! in `host/mod.rs`, so parallel contributors never edit shared code.

use mlua::{Lua, Result};

use crate::host::SaveHandle;

pub mod event_script;
pub mod field;
pub mod message;
pub mod save;

/// Every subsystem that contributes real bindings, in install order.
pub type SubsystemInit = fn(&Lua, &SaveHandle) -> Result<()>;

pub const SUBSYSTEMS: &[(&str, SubsystemInit)] = &[
    ("save", save::install),
    ("field", field::install),
    ("event_script", event_script::install),
    ("message", message::install),
];
