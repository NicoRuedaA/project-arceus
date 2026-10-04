//! Script layer: the game ships its event logic as Haxe-compiled Lua 5.3
//! bytecode (`bin/haxe/release/event/*.blua`). This module owns the Lua state
//! and chunk loading; the Haxe runtime environment is a separate work unit.

use mlua::{Lua, Result};

/// Create the Lua 5.3 state used by the script layer.
pub fn new_state() -> Lua {
    Lua::new()
}

/// Load a Lua 5.3 chunk (source or bytecode) without executing it.
pub fn load_chunk(lua: &Lua, chunk: &[u8]) -> Result<mlua::Function> {
    lua.load(chunk).into_function()
}

/// Execute a chunk. Until the Haxe runtime globals exist this is expected to
/// fail at runtime (not at load time).
pub fn exec_chunk(lua: &Lua, chunk: &[u8]) -> Result<()> {
    lua.load(chunk).exec()
}
