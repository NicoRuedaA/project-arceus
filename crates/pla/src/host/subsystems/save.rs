//! `save` subsystem — the real `Global.GetSaveSystem()` bindings.
//!
//! Reference implementation of the subsystem pattern (see `host/mod.rs`).
//! Scripts call these as methods (`SetEventFlag(self, flag)`), so the bindings
//! accept the variadic shape and pick the hash-like argument.

use mlua::{Lua, Result};

use crate::host::{stub_fallback, SaveHandle};
use crate::save::fnv1a64_str;

/// `Global.GetSaveSystem().EventWork():SetEventFlag(hash)` against the store.
pub fn install(lua: &Lua, save: &SaveHandle) -> Result<()> {
    fn hash_of(v: &mlua::Value) -> u64 {
        match v {
            // FnvHash64 values are stored as the exact bits in a signed
            // Lua integer (see host::install_fnv_hash).
            mlua::Value::Table(t) => t.get::<i64>("value").map(|v| v as u64).unwrap_or(0),
            mlua::Value::Integer(i) => *i as u64,
            mlua::Value::Number(n) => *n as u64,
            mlua::Value::String(s) => fnv1a64_str(&s.to_string_lossy()),
            _ => 0,
        }
    }

    // Scripts call these as methods: `SetEventFlag(self, flag)`, so accept the
    // variadic shape and pick the hash-like argument plus an optional boolean.
    let set_flag = {
        let save = save.clone();
        lua.create_function(move |_, args: mlua::MultiValue| {
            let mut id = 0u64;
            let mut found = false;
            let mut value = true;
            for a in args.iter() {
                if let mlua::Value::Boolean(b) = a {
                    value = *b;
                    continue;
                }
                if !found {
                    let h = hash_of(a);
                    if h != 0 {
                        id = h;
                        found = true;
                    }
                }
            }
            let mut s = save.borrow_mut();
            s.event_work.set(id, value);
            Ok(())
        })?
    };
    let get_flag = {
        let save = save.clone();
        lua.create_function(move |_, args: mlua::MultiValue| {
            for a in args.iter() {
                let h = hash_of(a);
                if h != 0 {
                    return Ok(save.borrow().event_work.get(h));
                }
            }
            Ok(false)
        })?
    };
    let event_work = lua.create_table()?;
    event_work.set("SetEventFlag", set_flag)?;
    event_work.set("GetEventFlag", get_flag)?;
    // N2 proves the save-chunk registry, but not these Lua method semantics.
    // Keep observed calls executable and traceable without guessing behavior.
    stub_fallback(lua, &event_work, "EventWork")?;

    let event_work_fn = {
        let event_work = event_work.clone();
        lua.create_function(move |_, _this: mlua::Value| Ok(event_work.clone()))?
    };
    let save_system = lua.create_table()?;
    save_system.set("EventWork", event_work_fn)?;
    // The save system exposes more chunks than the port has modeled. Unknown
    // accessors such as MyStatus remain recording stubs until their semantics
    // are supported by direct evidence.
    stub_fallback(lua, &save_system, "SaveSystem")?;

    let get_save_system = {
        let save_system = save_system.clone();
        lua.create_function(move |_, _: mlua::Value| Ok(save_system.clone()))?
    };
    let global = lua.create_table()?;
    global.set("GetSaveSystem", get_save_system)?;
    stub_fallback(lua, &global, "Global")?;
    lua.globals().set("Global", global)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_save_chunk_and_event_work_methods_are_recording_stubs() {
        let lua = Lua::new();
        crate::host::install(&lua).unwrap();
        lua.load(
            r#"
            local save = Global.GetSaveSystem()
            save:MyStatus():GetSex()
            save:EventWork():CheckEventFlag("story_flag")
            save:EventWork():ResetEventFlag("story_flag")
            save:EventWork():GetEventWork("story_work")
            "#,
        )
        .exec()
        .unwrap();

        let calls = crate::host::take_calls(&lua).unwrap();
        for expected in [
            "SaveSystem.MyStatus",
            "SaveSystem.MyStatus().GetSex",
            "EventWork.CheckEventFlag",
            "EventWork.ResetEventFlag",
            "EventWork.GetEventWork",
        ] {
            assert!(
                calls.iter().any(|call| call == expected),
                "missing {expected}: {calls:?}"
            );
        }
    }
}
