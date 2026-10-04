//! `event_script` subsystem — the real event-data chain.
//!
//! The event scripts resolve their own data through `Global`:
//!
//! ```lua
//! local data = Global.GetEventScriptManager()
//!   :GetEventProgressManager()
//!   :GetEventListManager()
//!   :FindEventData(scriptID)
//! local flag = FnvHash64.new(data:GetFlag())
//! if flag ~= FnvHash64.GetEmptyString() then
//!   Global.GetSaveSystem():EventWork():SetEventFlag(data:GetFlag())
//! end
//! ```
//!
//! `scriptID` is the event's own `FnvHash64` (the script sets it from its class
//! name via `SetScriptID`), and `GetFlag` returns the flag the event owns — the
//! same hash. The port therefore hands the script's own id back as the flag, so
//! the round trip lands in the real save system
//! (`FnvHash64.new` passes an existing hash through, and `SetEventFlag` stores
//! its `value`).
//!
//! The data fields the scripts read (`isFadeIn`, `isDemoSound`,
//! `isResetPlayerLookAt`, `waitMotionType`, `statusReset`, `standUp`,
//! `itemFadeOut`) are conservative defaults: `bin/event/event_progress/`
//! carries the real records but their layout is not decoded yet (dec037).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result, Value};

use crate::assets::event_list::EventList;
use crate::host::{stub_fallback, SaveHandle};

/// Extract the `FnvHash64.value` from a call's arguments (the script passes its
/// own id as the last argument of the method call).
fn hash_arg(args: &mlua::MultiValue) -> Option<mlua::Table> {
    args.iter().rev().find_map(|v| match v {
        // The value is the exact bits in a signed Lua integer.
        Value::Table(t) => t.get::<i64>("value").ok().map(|_| t.clone()),
        _ => None,
    })
}

pub fn install(lua: &Lua, _save: &SaveHandle) -> Result<()> {
    // The port-side event registry: `__port_load_event_list` parses the game's
    // `event_list.bin` and lets `FindEventData` name the event it was asked for.
    let registry: Rc<RefCell<Option<EventList>>> = Rc::new(RefCell::new(None));
    let list_manager = lua.create_table()?;

    list_manager.set("FindEventData", {
        let registry = registry.clone();
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let data = lua.create_table()?;
            let id = hash_arg(&args);
            // Real event identity when the registry is loaded; an empty name
            // means the event list does not know this id.
            let mut name = String::new();
            {
                let guard = registry.borrow();
                if let (Some(list), Some(id)) = (guard.as_ref(), id.as_ref()) {
                    if let Ok(v) = id.get::<i64>("value") {
                        if let Some(found) = list.find_by_hash(v as u64) {
                            name = found.to_string();
                        }
                    }
                }
            }
            data.set("name", name)?;
            // The event's flag is its own id (see the module docs). The getter
            // returns the very hash the script passed in, so
            // `FnvHash64.new(GetFlag())` and `SetEventFlag(GetFlag())` agree.
            let flag = id.clone();
            data.set(
                "__haxe_export_GetFlag",
                lua.create_function(move |_, _this: Value| Ok(flag.clone()))?,
            )?;
            // Fields the reference event reads; conservative defaults until the
            // event records are decoded.
            data.set("isFadeIn", false)?;
            data.set("isDemoSound", false)?;
            data.set("isResetPlayerLookAt", false)?;
            data.set("waitMotionType", 0)?;
            data.set("statusReset", false)?;
            data.set("standUp", false)?;
            data.set("itemFadeOut", false)?;
            stub_fallback(lua, &data, "EventCommonData")?;
            Ok(data)
        })?
    })?;
    stub_fallback(lua, &list_manager, "EventListManager")?;

    let progress_manager = lua.create_table()?;
    let list_for_getter = list_manager.clone();
    progress_manager.set(
        "GetEventListManager",
        lua.create_function(move |_, _: mlua::MultiValue| Ok(list_for_getter.clone()))?,
    )?;
    stub_fallback(lua, &progress_manager, "EventProgressManager")?;

    let script_manager = lua.create_table()?;
    let progress_for_getter = progress_manager.clone();
    script_manager.set(
        "GetEventProgressManager",
        lua.create_function(move |_, _: mlua::MultiValue| Ok(progress_for_getter.clone()))?,
    )?;
    {
        let registry = registry.clone();
        script_manager.set(
            "__port_load_event_list",
            // Method-call convention: `manager:__port_load_event_list(data)`.
            lua.create_function(move |_, (_this, data): (Value, mlua::LuaString)| {
                let bytes = data.as_bytes();
                let list = EventList::parse(&bytes).map_err(mlua::Error::RuntimeError)?;
                let n = list.len();
                *registry.borrow_mut() = Some(list);
                Ok(n)
            })?,
        )?;
    }
    stub_fallback(lua, &script_manager, "EventScriptManager")?;

    let get_manager = {
        let manager = script_manager.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(manager.clone()))?
    };

    let global: mlua::Table = match lua.globals().get("Global") {
        Ok(t) => t,
        Err(_) => {
            let t = lua.create_table()?;
            lua.globals().set("Global", t.clone())?;
            t
        }
    };
    global.set("GetEventScriptManager", get_manager)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_event_data_returns_the_script_flag() {
        let lua = Lua::new();
        crate::host::install(&lua).unwrap();
        lua.load(
            r#"
            local id = FnvHash64.new("BoutiqueMainEvent")
            local data = Global.GetEventScriptManager()
              :GetEventProgressManager()
              :GetEventListManager()
              :FindEventData(id)
            assert(data.isFadeIn == false)
            assert(data.isDemoSound == false)
            local flag = FnvHash64.new(data:__haxe_export_GetFlag())
            assert(flag ~= FnvHash64.GetEmptyString())
            assert(flag.value == id.value)
            "#,
        )
        .exec()
        .unwrap();
    }
}
