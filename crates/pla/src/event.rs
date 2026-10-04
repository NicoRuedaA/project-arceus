//! Event system: the decoded `ScriptCall(class, module, target)` protocol.
//!
//! The game runs its event logic as Haxe-compiled Lua 5.3 bytecode. The engine
//! calls `system.EventScriptMain.ScriptCall(class, module, target)`; the script
//! resolves `<module>.<class>`, binds the target to a `ScriptObjectHandle`,
//! initializes the ten command singletons and runs `Execute`.
//!
//! `Execute` waits by yielding (`coroutine.yield`), so the engine resumes the
//! event once per frame. The port drives the same way: the call runs inside a
//! coroutine and [`run`] resumes it up to a frame budget.
//!
//! `target` is the engine-side object the scripts see as
//! `ScriptObjectHandle.m_scriptObject`; the real binary registers it as
//! `event_script::EventScriptObject` with exactly `GetTargetID` and `IsSkip`.
//! [`crate::host::make_event_target`] builds the port's equivalent.

use mlua::thread::ThreadStatus;
use mlua::{Function, Lua, MultiValue, Result, Table, Value};

/// How an event run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    /// The script returned (event finished or took its skip path).
    Finished,
    /// The script is still waiting after the frame budget was consumed.
    FrameLimit,
}

/// One event run: outcome, resumes consumed, and the value `ScriptCall`
/// returned (0 on the script's own failure path, the event result otherwise).
#[derive(Debug)]
pub struct EventRun {
    pub outcome: EventOutcome,
    /// Coroutine resumes performed (`1` = ran without waiting).
    pub frames: usize,
    pub result: Value,
}

/// Resolve `system.EventScriptMain.ScriptCall` from a loaded script module.
pub fn resolve_entry(module: &Table) -> Result<Function> {
    let system: Table = module.get("system")?;
    let main: Table = system.get("EventScriptMain")?;
    main.get("ScriptCall")
}

/// Run one event inside a coroutine, resuming it at most `max_frames` times.
///
/// A script-side failure is swallowed by the script's own `pcall` (the game
/// treats a failed event as "not started"), so the returned `result` is the
/// observable outcome and the host-call trace shows how far it got.
pub fn run(
    lua: &Lua,
    module: &Table,
    class: &str,
    module_name: &str,
    target: Table,
    max_frames: usize,
) -> Result<EventRun> {
    let entry = resolve_entry(module)?;
    // The coroutine body must be a Lua function: the event waits with
    // `coroutine.yield()` from inside its own `pcall`, and yielding across the
    // Rust closure's C-call boundary would abort the event.
    let body = lua
        .load("local entry, class, module_name, target = ...\nreturn entry(class, module_name, target)")
        .set_name("event-body")
        .into_function()?;
    let thread = lua.create_thread(body)?;
    let mut frames = 0usize;
    let mut first = Some((entry, class.to_string(), module_name.to_string(), target));
    loop {
        let resumed = match first.take() {
            Some(args) => thread.resume::<MultiValue>(args),
            None => thread.resume::<MultiValue>(()),
        };
        match resumed {
            Ok(values) => {
                if thread.status() == ThreadStatus::Finished {
                    let result = values.into_iter().next().unwrap_or(Value::Nil);
                    return Ok(EventRun {
                        outcome: EventOutcome::Finished,
                        frames: frames + 1,
                        result,
                    });
                }
                frames += 1;
                if frames >= max_frames {
                    return Ok(EventRun {
                        outcome: EventOutcome::FrameLimit,
                        frames,
                        result: Value::Nil,
                    });
                }
            }
            Err(mlua::Error::CoroutineUnresumable) => {
                return Ok(EventRun {
                    outcome: EventOutcome::Finished,
                    frames,
                    result: Value::Nil,
                });
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_EVENT: &str = r#"
system = { EventScriptMain = {} }
function system.EventScriptMain.ScriptCall(class, module, target)
  local n = 0
  while n < 3 do
    n = n + 1
    coroutine.yield()
  end
  return 7
end
return { system = system }
"#;

    #[test]
    fn drives_a_yielding_event_to_completion() {
        let lua = Lua::new();
        let module: Table = lua.load(FAKE_EVENT).eval().unwrap();
        let target = lua.create_table().unwrap();
        let run = run(&lua, &module, "Any", "general", target, 16).unwrap();
        assert_eq!(run.outcome, EventOutcome::Finished);
        assert_eq!(run.frames, 4, "three yields plus the final resume");
        assert!(matches!(run.result, Value::Integer(7)));
    }

    #[test]
    fn reports_the_frame_limit_for_a_waiting_event() {
        let lua = Lua::new();
        let module: Table = lua.load(FAKE_EVENT).eval().unwrap();
        let target = lua.create_table().unwrap();
        let run = run(&lua, &module, "Any", "general", target, 2).unwrap();
        assert_eq!(run.outcome, EventOutcome::FrameLimit);
        assert_eq!(run.frames, 2);
    }
}
