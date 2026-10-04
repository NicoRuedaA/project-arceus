//! End-to-end event script execution through the host layer.
//!
//! Proves the decoded protocol works: a real Haxe-compiled event script loads
//! as Lua 5.3, its `system.EventScriptMain.ScriptCall` entry resolves the event
//! class, the script binds the engine target (the port's
//! `event_script::EventScriptObject` equivalent), drives the host API
//! (`Global.GetSaveSystem`, `GetEventScriptManager`, message window, ...) and
//! waits by yielding until the engine resumes it.
//!
//! The script fixture is game content and lives outside the repository (see
//! `tests/common/mod.rs`); when it is missing the tests skip.

mod common;

use common::fixture;
use mlua::Value;
use pla::event::{self, EventOutcome};
use pla::host;
use pla::script::{load_chunk, new_state};

struct Run {
    outcome: EventOutcome,
    result: Value,
    calls: Vec<String>,
}

fn load_module(lua: &mlua::Lua) -> Option<mlua::Table> {
    let bytes = fixture("event_sample.blua")?;
    let module: mlua::Table = load_chunk(lua, &bytes)
        .expect("chunk loads")
        .call(())
        .expect("chunk executes");
    Some(module)
}

/// Run the reference event with the port's engine target; `skip` flips the
/// player-skip state the event polls through `ScriptObjectHandle.IsSkip`.
fn run_event(lua: &mlua::Lua, class: &str, skip: bool) -> Option<Run> {
    let module = load_module(lua)?;
    let target = host::make_event_target(lua, 4242).unwrap();
    target.set("_skip", skip).unwrap();
    let run = event::run(lua, &module, class, "general", target, 256).expect("event runs");
    let calls = host::take_calls(lua).unwrap();
    Some(Run {
        outcome: run.outcome,
        result: run.result,
        calls,
    })
}

#[test]
fn event_script_runs_to_completion() {
    let lua = new_state();
    host::install(&lua).unwrap();
    let Some(run) = run_event(&lua, "BoutiqueMainEvent", false) else {
        return;
    };

    // The reference event now runs end to end: the field subsystem answers its
    // load waits, so it does not park in a frame loop, and `ScriptCall` returns
    // its success value (0).
    assert_eq!(
        run.outcome,
        EventOutcome::Finished,
        "trace: {:?}",
        run.calls
    );
    assert!(
        matches!(run.result, Value::Integer(0)),
        "ScriptCall returns 0 on success, got {:?}",
        run.result
    );
    assert!(
        run.calls.iter().any(|c| c.starts_with("Global.")),
        "expected host traffic, got: {:?}",
        run.calls
    );
    assert!(
        run.calls.len() >= 20,
        "expected a real trace, got {} calls: {:?}",
        run.calls.len(),
        run.calls
    );
    // The remaining trace is the work queue: only unported bindings record.
    println!("host trace ({} calls):", run.calls.len());
    for (i, call) in run.calls.iter().enumerate() {
        println!("{i:4} {call}");
    }
}

#[test]
fn event_script_sets_a_flag_in_the_save_system() {
    let lua = new_state();
    let save = host::install(&lua).unwrap();
    assert_eq!(
        save.borrow().event_work.known(),
        450,
        "save system should be seeded from the generated flag table"
    );

    let Some(run) = run_event(&lua, "BoutiqueMainEvent", false) else {
        return;
    };
    println!("host trace ({} calls): {:?}", run.calls.len(), run.calls);

    // The event reached its flag logic and wrote through the real save system:
    // 450 seeded flags plus the one it set.
    let s = save.borrow();
    assert!(
        s.event_work.writes >= 1,
        "no SetEventFlag call reached the save system: {:?}",
        run.calls
    );
    for id in &s.event_work.set_flags() {
        match s.event_work.name(*id) {
            Some(name) => println!("flag {id:#018x} = {name}"),
            None => {
                println!("flag {id:#018x} (not in event_flags.tbl; hash of the stub flag value)")
            }
        }
    }
    assert!(s.event_work.known() >= 451);
}

#[test]
fn skip_state_stops_the_event_early() {
    let lua = new_state();
    host::install(&lua).unwrap();
    let Some(normal) = run_event(&lua, "BoutiqueMainEvent", false) else {
        return;
    };

    let lua = new_state();
    host::install(&lua).unwrap();
    let Some(skipped) = run_event(&lua, "BoutiqueMainEvent", true) else {
        return;
    };

    // Both runs start identically (the flag logic runs before the first skip
    // poll); the skipped run leaves the wait loop on the first poll and takes
    // the script's skip path.
    assert!(
        skipped.calls.len() < normal.calls.len(),
        "expected the skipped run to do less work:\nnormal  ({} calls): {:?}\nskipped ({} calls): {:?}",
        normal.calls.len(),
        normal.calls,
        skipped.calls.len(),
        skipped.calls
    );
}

#[test]
fn event_flag_is_keyed_by_the_event_id() {
    let lua = new_state();
    let save = host::install(&lua).unwrap();
    let Some(run) = run_event(&lua, "BoutiqueMainEvent", false) else {
        return;
    };
    // The event-data chain is real now: FindEventData is a binding, not a stub.
    assert!(
        !run.calls.iter().any(|c| c.contains("FindEventData")),
        "FindEventData should be a real binding now: {:?}",
        run.calls
    );
    // The flag it sets is its own id (the FNV hash of the event class name):
    // FindEventData(scriptID):GetFlag() -> SetEventFlag(scriptID).
    let expected = pla::save::fnv1a64_str("BoutiqueMainEvent");
    let s = save.borrow();
    assert!(
        s.event_work.set_flags().contains(&expected),
        "expected the event's own id {expected:#018x} in {:?}",
        s.event_work.set_flags()
    );
}

#[test]
fn unknown_class_returns_zero() {
    let lua = new_state();
    host::install(&lua).unwrap();
    let Some(run) = run_event(&lua, "NoSuchEvent", false) else {
        return;
    };
    // The script's own pcall guard swallows the failure and returns 0; the host
    // should not have seen any event-specific traffic.
    assert!(
        run.calls.iter().all(|c| !c.contains("GetSaveSystem")),
        "unexpected calls for unknown class: {:?}",
        run.calls
    );
    assert!(matches!(run.result, Value::Integer(0)));
}

#[test]
fn event_registry_names_the_event_by_its_id() {
    let lua = new_state();
    host::install(&lua).unwrap();
    let Some(bytes) = fixture("event_list.bin") else {
        return;
    };
    let data = lua.create_string(&bytes).unwrap();
    let loaded: usize = lua
        .load(
            "local data = ...; return Global.GetEventScriptManager():__port_load_event_list(data)",
        )
        .call(data)
        .unwrap();
    assert!(loaded > 1_800, "loaded {loaded} names");

    // `FindEventData` now names the event behind the script id.
    let name: Option<String> = lua
        .load(
            r#"
            local id = FnvHash64.new("TutorialNPC_area03_001")
            local data = Global.GetEventScriptManager()
              :GetEventProgressManager()
              :GetEventListManager()
              :FindEventData(id)
            return data.name
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(name.as_deref(), Some("TutorialNPC_area03_001"));

    // An event the list does not know stays anonymous but still works.
    let unknown: Option<String> = lua
        .load(
            r#"
            local id = FnvHash64.new("NotInTheListEvent")
            local data = Global.GetEventScriptManager()
              :GetEventProgressManager()
              :GetEventListManager()
              :FindEventData(id)
            return data.name
            "#,
        )
        .eval()
        .unwrap();
    assert_eq!(unknown.as_deref(), Some(""));
}
