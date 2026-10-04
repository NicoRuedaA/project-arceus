//! Script layer: Lua 5.3 VM plus the game's Haxe-compiled bytecode.
//!
//! The event script fixture is game content and lives outside the repository
//! (see `tests/common/mod.rs`); when it is missing the bytecode tests skip.

mod common;

use common::fixture;
use pla::script::{exec_chunk, load_chunk, new_state};

#[test]
fn lua_vm_runs_source() {
    let lua = new_state();
    let sum: i64 = lua.load("return 1 + 1").eval().unwrap();
    assert_eq!(sum, 2);
}

#[test]
fn game_bytecode_loads_as_lua53() {
    let Some(bytes) = fixture("event_sample.blua") else {
        return;
    };
    let lua = new_state();
    // A Haxe-compiled event script: loading proves Lua 5.3 chunk compatibility.
    let chunk = load_chunk(&lua, &bytes);
    assert!(chunk.is_ok(), "chunk failed to load: {:?}", chunk.err());
}

#[test]
fn game_bytecode_executes_until_the_haxe_runtime_is_needed() {
    let Some(bytes) = fixture("event_sample.blua") else {
        return;
    };
    let lua = new_state();
    let result = exec_chunk(&lua, &bytes);
    // Expected to fail on a missing Haxe runtime global; the point is that the
    // failure is a runtime error, not a load/version error.
    match result {
        Ok(()) => {}
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("nil") || msg.contains("global") || msg.contains("attempt"),
                "unexpected error shape: {msg}"
            );
        }
    }
}
