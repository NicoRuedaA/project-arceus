//! Host layer for the Lua script environment — **frozen interface**.
//!
//! The engine exposes a Lua API to the game's Haxe-compiled scripts. This
//! module owns the parts that never change per subsystem:
//!
//! * `PRELUDE` — the Lua-side scaffolding: the `bit` shim the Haxe runtime
//!   needs, callable recording stubs, `ScriptObjectHandle` and the ten command
//!   singletons.
//! * [`install`] — builds a Lua state and installs every registered subsystem.
//! * [`stub_fallback`] — lets a partially ported table fall back to recording
//!   stubs, so scripts keep running and show what is still missing.
//! * [`take_calls`] — reads (and clears) the recorded host-call trace.
//!
//! ## Adding a subsystem (the only supported extension point)
//!
//! 1. Create `host/subsystems/<name>.rs` with
//!    `pub fn install(lua: &Lua, save: &SaveHandle) -> Result<()>`, using
//!    [`stub_fallback`] for the parts that are not ported yet.
//! 2. Add one line to `subsystems::SUBSYSTEMS`.
//!
//! Nothing else in this file changes: the interface is frozen so several
//! collaborators can add subsystems in parallel without touching shared code.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result};

use crate::save::SaveSystem;

pub mod subsystems;

/// Shared handle to the port's save system (the scripts mutate it).
pub type SaveHandle = Rc<RefCell<SaveSystem>>;

/// The ten command singletons the engine binds to a script object handle.
pub const COMMANDS: &[&str] = &[
    "FieldObjectCommand",
    "AppCommand",
    "CameraCommand",
    "CommonCommand",
    "FadeCommand",
    "ItemCommand",
    "MessageCommand",
    "PokemonCommand",
    "RealTimeEventCommand",
    "SoundCommand",
];

const PRELUDE: &str = r#"
-- Recording log shared with Rust.
__host_calls = {}

-- LuaJIT-style `bit` library, required by the Haxe Lua runtime.
if not bit then
  local function tobit(x)
    x = math.floor(x)
    if x < 0 then x = x + 4294967296 end
    return x % 4294967296
  end
  local function band(a, b)
    a, b = tobit(a), tobit(b)
    local r, p = 0, 1
    for _ = 1, 32 do
      local x, y = a % 2, b % 2
      if x == 1 and y == 1 then r = r + p end
      a, b, p = (a - x) / 2, (b - y) / 2, p * 2
    end
    return r
  end
  local function bor(a, b)
    a, b = tobit(a), tobit(b)
    local r, p = 0, 1
    for _ = 1, 32 do
      local x, y = a % 2, b % 2
      if x == 1 or y == 1 then r = r + p end
      a, b, p = (a - x) / 2, (b - y) / 2, p * 2
    end
    return r
  end
  local function bxor(a, b)
    a, b = tobit(a), tobit(b)
    local r, p = 0, 1
    for _ = 1, 32 do
      local x, y = a % 2, b % 2
      if x ~= y then r = r + p end
      a, b, p = (a - x) / 2, (b - y) / 2, p * 2
    end
    return r
  end
  bit = {
    band = band,
    bor = bor,
    bxor = bxor,
    bnot = function(a) return 4294967295 - tobit(a) end,
    lshift = function(a, n) return tobit(tobit(a) * 2 ^ n) end,
    rshift = function(a, n) return math.floor(tobit(a) / 2 ^ n) end,
    arshift = function(a, n)
      a = tobit(a)
      if a >= 2147483648 then a = a - 4294967296 end
      return tobit(math.floor(a / 2 ^ n))
    end,
    bswap = function(a)
      a = tobit(a)
      local b0 = a % 256; a = (a - b0) / 256
      local b1 = a % 256; a = (a - b1) / 256
      local b2 = a % 256; a = (a - b2) / 256
      return ((a % 256) + b2 * 256 + b1 * 65536 + b0 * 16777216) % 4294967296
    end,
    tobit = tobit,
    tohex = function(a) return string.format("%08x", tobit(a)) end,
  }
  package.loaded["bit"] = bit
end

-- Callable recording stub: any field access and any call is logged.
local function make_stub(name)
  local stub
  stub = setmetatable({}, {
    __index = function(_, k) return make_stub(name .. "." .. tostring(k)) end,
    __call = function(_, ...)
      table.insert(__host_calls, name)
      -- Return another stub so chained field access keeps working while the
      -- port has no real implementation yet.
      return make_stub(name .. "()")
    end,
    __tostring = function() return name end,
  })
  return stub
end

__make_stub = make_stub

-- ScriptObjectHandle.new(target) -> handle.
-- The game's scripts define their own `stock.ScriptObjectHandle` (Haxe) and
-- delegate `IsSkip`/`GetTargetID` to the engine target; this global keeps the
-- same contract for any script that reaches for the engine global instead.
ScriptObjectHandle = {}
ScriptObjectHandle.new = function(target)
  table.insert(__host_calls, "ScriptObjectHandle.new")
  local h = { target = target }
  h.GetTargetID = function()
    if target and target.GetTargetID then return target.GetTargetID() end
    return 0
  end
  h.IsSkip = function()
    if target and target.IsSkip then return target.IsSkip() end
    return false
  end
  return setmetatable(h, {
    __index = function(_, k)
      return make_stub("ScriptObjectHandle." .. tostring(k))
    end,
  })
end

-- Engine globals the scripts reference directly (census of 797 scripts).
-- FnvHash64 and Global are real bindings (installed from Rust); the rest stay
-- recording stubs until their subsystems are ported.
for _, name in ipairs({
  "FnvHash", "Utility", "Boot", "Quaternion",
  "Vector2", "Vector3", "Vector4", "Matrix44", "Random", "GameManager",
  "SoundManager", "Time", "Debug",
}) do
  _G[name] = make_stub(name)
end

-- The ten command singletons.
for _, name in ipairs({
  "FieldObjectCommand", "AppCommand", "CameraCommand", "CommonCommand",
  "FadeCommand", "ItemCommand", "MessageCommand", "PokemonCommand",
  "RealTimeEventCommand", "SoundCommand",
}) do
  _G[name] = make_stub(name)
end

-- Any other engine global resolves to a recording stub on first read, so the
-- port never hard-fails on an unported binding and the trace shows what was
-- asked for. (The census over the 799 scripts lists the common ones above;
-- scripts also read AppConfig, SoundController, FieldUtility, engine classes
-- such as CameraParameter, and more.)
setmetatable(_G, {
  __index = function(t, k)
    local s = make_stub(tostring(k))
    rawset(t, k, s)
    return s
  end,
})
"#;

/// Install the host layer: the Lua prelude, the engine-wide globals and every
/// registered subsystem. Returns the shared save system so callers can observe
/// what scripts did.
pub fn install(lua: &Lua) -> Result<SaveHandle> {
    lua.load(PRELUDE).set_name("host-prelude").exec()?;
    let save: SaveHandle = Rc::new(RefCell::new(SaveSystem::seeded()));
    install_fnv_hash(lua)?;
    for (name, init) in subsystems::SUBSYSTEMS {
        init(lua, &save)
            .map_err(|e| mlua::Error::RuntimeError(format!("subsystem {name}: {e}")))?;
    }
    Ok(save)
}

/// `FnvHash64` with value equality: `new(str)`, `GetEmptyString()`.
///
/// The 64-bit value is stored as a Lua **integer holding the exact bits**
/// (`i64::from_le_bytes`): Lua 5.3 integers are signed, so values above
/// `i64::MAX` would otherwise become doubles and silently lose the low 11
/// bits. Consumers read the field back as `i64` and reinterpret it as `u64`.
fn install_fnv_hash(lua: &Lua) -> Result<()> {
    let mt = lua.create_table()?;
    mt.set(
        "__eq",
        lua.create_function(|_, (a, b): (mlua::Table, mlua::Table)| {
            let av: i64 = a.get("value")?;
            let bv: i64 = b.get("value")?;
            Ok(av == bv)
        })?,
    )?;
    mt.set(
        "__tostring",
        lua.create_function(|_, t: mlua::Table| {
            let v: i64 = t.get("value")?;
            Ok(format!("FnvHash64({:#018x})", v as u64))
        })?,
    )?;

    let hash_new = {
        let mt = mt.clone();
        lua.create_function(move |lua, value: mlua::Value| {
            // `FnvHash64.new` also accepts an existing hash (the event scripts
            // wrap `EventData:GetFlag()`'s result in it): pass it through.
            if let mlua::Value::Table(src) = &value {
                if let Ok(v) = src.get::<i64>("value") {
                    let text = src.get::<String>("text").unwrap_or_default();
                    let t = lua.create_table()?;
                    t.set("value", v)?;
                    t.set("text", text)?;
                    let _ = t.set_metatable(Some(mt.clone()));
                    return Ok(t);
                }
            }
            let text = match &value {
                mlua::Value::String(s) => s.to_string_lossy().to_string(),
                other => format!("{other:?}"),
            };
            let t = lua.create_table()?;
            t.set("value", crate::save::fnv1a64_str(&text) as i64)?;
            t.set("text", text)?;
            let _ = t.set_metatable(Some(mt.clone()));
            Ok(t)
        })?
    };
    let empty = lua.create_table()?;
    empty.set("value", crate::save::fnv1a64_str("") as i64)?;
    empty.set("text", "")?;
    empty.set_metatable(Some(mt))?;

    let get_empty = {
        let empty = empty.clone();
        lua.create_function(move |_, _: mlua::Value| Ok(empty.clone()))?
    };
    let fh = lua.create_table()?;
    fh.set("new", hash_new)?;
    fh.set("GetEmptyString", get_empty)?;
    stub_fallback(lua, &fh, "FnvHash64")?;
    lua.globals().set("FnvHash64", fh)
}

/// Unknown methods on a real table fall back to recording stubs, so a partially
/// ported subsystem still lets the script run (and shows what is missing).
pub(crate) fn stub_fallback(lua: &Lua, table: &mlua::Table, prefix: &str) -> Result<()> {
    let mt = lua.create_table()?;
    let prefix = prefix.to_string();
    mt.set(
        "__index",
        lua.create_function(move |lua, (_t, key): (mlua::Table, mlua::LuaString)| {
            let name = format!("{}.{}", prefix, key.to_string_lossy());
            let make: mlua::Function = lua.globals().get("__make_stub")?;
            make.call::<mlua::Value>(name)
        })?,
    )?;
    table.set_metatable(Some(mt))
}

/// Build the engine-side object an event script binds to (the real binary's
/// `event_script::EventScriptObject`: exactly `GetTargetID` and `IsSkip`).
///
/// The script-side `stock.ScriptObjectHandle` stores it as `m_scriptObject`
/// and delegates both methods to it. `_skip` is a readable field so the driver
/// can flip the player-skip state; `_target_id` backs `GetTargetID`.
pub fn make_event_target(lua: &Lua, target_id: i64) -> Result<mlua::Table> {
    let target = lua.create_table()?;
    target.set("_target_id", target_id)?;
    target.set("_skip", false)?;
    target.set(
        "GetTargetID",
        lua.create_function(|_, t: mlua::Table| t.get::<i64>("_target_id"))?,
    )?;
    target.set(
        "IsSkip",
        lua.create_function(|_, t: mlua::Table| t.get::<bool>("_skip"))?,
    )?;
    stub_fallback(lua, &target, "EventScriptObject")?;
    Ok(target)
}

/// Read and clear the recorded host calls.
pub fn take_calls(lua: &Lua) -> Result<Vec<String>> {
    let log: mlua::Table = lua.globals().get("__host_calls")?;
    let mut calls = Vec::new();
    for value in log.sequence_values::<String>() {
        calls.push(value?);
    }
    log.clear()?;
    Ok(calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_falls_back_to_stubs() {
        let lua = Lua::new();
        install(&lua).unwrap();
        // Any global the port has not ported yet resolves to a recording stub.
        let v: mlua::Value = lua
            .load("return Global.GetSomethingNotPortedYet()")
            .eval()
            .unwrap();
        assert!(matches!(v, mlua::Value::Table(_)), "got {v:?}");
        let calls = take_calls(&lua).unwrap();
        assert!(
            calls.iter().any(|c| c.contains("GetSomethingNotPortedYet")),
            "calls: {calls:?}"
        );
    }

    #[test]
    fn fnv_hash_equality_and_empty() {
        let lua = Lua::new();
        install(&lua).unwrap();
        let same: bool = lua
            .load("return FnvHash64.new('a') == FnvHash64.new('a')")
            .eval()
            .unwrap();
        assert!(same);
        let diff: bool = lua
            .load("return FnvHash64.new('a') == FnvHash64.GetEmptyString()")
            .eval()
            .unwrap();
        assert!(!diff);
    }

    #[test]
    fn installs_bit_and_commands() {
        let lua = Lua::new();
        install(&lua).unwrap();
        let band: i64 = lua.load("return bit.band(12, 10)").eval().unwrap();
        assert_eq!(band, 8);
        let require: i64 = lua.load("return require('bit').bor(1, 2)").eval().unwrap();
        assert_eq!(require, 3);
        let call: bool = lua
            .load("FieldObjectCommand.Initialize(); return true")
            .eval()
            .unwrap();
        assert!(call);
        let calls = take_calls(&lua).unwrap();
        assert_eq!(calls, vec!["FieldObjectCommand.Initialize"]);
    }
}
