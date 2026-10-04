//! `message` subsystem — the real `Global.GetFieldMessageWindowManager()`.
//!
//! The event scripts reach the message window through `Global`:
//!
//! ```lua
//! Global.GetFieldMessageWindowManager():WordSetPlayerName(name)
//! Global.GetFieldMessageWindowManager():IsClosedMessageWindow()
//! Global.GetFieldMessageWindowManager():CloseMessageWindow()
//! ```
//!
//! The port owns the window state: `WordSetPlayerName` records the name (kept
//! on the manager table itself, so tests can read it back), a headless port has
//! no open window (`IsClosedMessageWindow` = true) and closing is a no-op.
//! Everything else falls back to recording stubs.
//!
//! The port also owns the text: `__port_load_messages(tbl, dat, keystream)`
//! loads a message file plus the recovered per-index keys, and
//! `GetMessage(key)` returns the decoded string (dec046).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result, Value};

use crate::assets::message::{MessageKeystream, MessageStore};
use crate::host::{stub_fallback, SaveHandle};

/// A loaded `.tbl` + `.dat` pair, its raw bytes and the recovered keys.
type Loaded = Rc<RefCell<Option<(MessageStore, Vec<u8>, MessageKeystream)>>>;

pub fn install(lua: &Lua, _save: &SaveHandle) -> Result<()> {
    let manager = lua.create_table()?;
    // Loaded message file + keys (port side; the scripts never see this).
    let loaded: Loaded = Rc::new(RefCell::new(None));

    {
        let loaded = loaded.clone();
        manager.set(
            "__port_load_messages",
            lua.create_function(
                move |_,
                      (_this, tbl, dat, keystream): (
                    Value,
                    mlua::LuaString,
                    mlua::LuaString,
                    mlua::LuaString,
                )| {
                    let store =
                        MessageStore::parse(tbl.as_bytes().as_ref(), dat.as_bytes().as_ref())
                            .map_err(mlua::Error::RuntimeError)?;
                    let keys = MessageKeystream::parse(&keystream.to_string_lossy())
                        .map_err(mlua::Error::RuntimeError)?;
                    let n = store.count();
                    *loaded.borrow_mut() = Some((store, dat.as_bytes().to_vec(), keys));
                    Ok(n)
                },
            )?,
        )?;
    }
    {
        let loaded = loaded.clone();
        manager.set(
            "GetMessage",
            lua.create_function(move |_, (_this, key): (Value, mlua::LuaString)| {
                let key = key.to_string_lossy();
                let guard = loaded.borrow();
                let Some((store, dat, keys)) = guard.as_ref() else {
                    return Ok(None);
                };
                Ok(keys.decode_key(store, dat, &key))
            })?,
        )?;
    }

    // Scripts call it as a method: `m:WordSetPlayerName(name)` or
    // `m.WordSetPlayerName(m, name)`; take the last string argument.
    let store = manager.clone();
    manager.set(
        "WordSetPlayerName",
        lua.create_function(move |_, args: mlua::MultiValue| {
            let name = args.iter().rev().find_map(|v| match v {
                Value::String(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            });
            if let Some(name) = name {
                store.set("player_name", name)?;
            }
            Ok(())
        })?,
    )?;

    // Real text: `SetMessage(key)` resolves the key through the loaded file and
    // puts the decoded string on the manager (`text`), which is what a
    // renderer would show. `IsClosedMessageWindow` reports whether a message is
    // pending, so the scripts' message waits behave.
    let pending: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    {
        let loaded = loaded.clone();
        let pending = pending.clone();
        manager.set(
            "SetMessage",
            lua.create_function(move |lua, (_this, key): (Value, mlua::LuaString)| {
                let key = key.to_string_lossy();
                let guard = loaded.borrow();
                let text = match guard.as_ref() {
                    Some((store, dat, keys)) => keys.decode_key(store, dat, &key),
                    None => None,
                };
                drop(guard);
                match text {
                    Some(text) => {
                        *pending.borrow_mut() = true;
                        Ok(Value::String(lua.create_string(&text)?))
                    }
                    None => Ok(Value::Nil),
                }
            })?,
        )?;
    }
    {
        let loaded = loaded.clone();
        manager.set(
            "WordSetMessage",
            lua.create_function(move |lua, (_this, key): (Value, mlua::LuaString)| {
                let key = key.to_string_lossy();
                let guard = loaded.borrow();
                let text = match guard.as_ref() {
                    Some((store, dat, keys)) => keys.decode_key(store, dat, &key),
                    None => None,
                };
                drop(guard);
                text.map(|t| lua.create_string(&t)).transpose()
            })?,
        )?;
    }
    manager.set(
        "IsEndMessage",
        lua.create_function({
            let pending = pending.clone();
            move |_, _: mlua::MultiValue| Ok(!*pending.borrow())
        })?,
    )?;
    manager.set(
        "IsClosedMessageWindow",
        lua.create_function({
            let pending = pending.clone();
            move |_, _: mlua::MultiValue| Ok(!*pending.borrow())
        })?,
    )?;
    manager.set(
        "CloseMessageWindow",
        lua.create_function({
            let pending = pending.clone();
            move |_, _: mlua::MultiValue| {
                *pending.borrow_mut() = false;
                Ok(())
            }
        })?,
    )?;
    manager.set(
        "GetContextmenu",
        lua.create_function(|_, _: mlua::MultiValue| Ok(Value::Nil))?,
    )?;
    stub_fallback(lua, &manager, "FieldMessageWindowManager")?;

    let get_manager = {
        let manager = manager.clone();
        lua.create_function(move |_, _: mlua::MultiValue| Ok(manager.clone()))?
    };

    // `Global` is created by the save subsystem (registered first); create it
    // defensively so the subsystem also works standalone.
    let global: mlua::Table = match lua.globals().get("Global") {
        Ok(t) => t,
        Err(_) => {
            let t = lua.create_table()?;
            lua.globals().set("Global", t.clone())?;
            t
        }
    };
    global.set("GetFieldMessageWindowManager", get_manager)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_the_player_name_and_reports_a_closed_window() {
        let lua = Lua::new();
        crate::host::install(&lua).unwrap();
        lua.load(
            r#"
            local m = Global.GetFieldMessageWindowManager()
            m:WordSetPlayerName("Nico")
            assert(m:IsClosedMessageWindow() == true)
            m:CloseMessageWindow()
            assert(m:GetContextmenu() == nil)
            "#,
        )
        .exec()
        .unwrap();
        let name: String = lua
            .load("return Global.GetFieldMessageWindowManager().player_name")
            .eval()
            .unwrap();
        assert_eq!(name, "Nico");
    }
}
