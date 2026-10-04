//! Run a Haxe-compiled Lua 5.3 event script and probe its host API needs.
//!
//! Usage: cargo run -p pla --example run_script -- <file.blua> [--run] [--probe]
//!
//! `--run` installs the host layer, builds the engine target and drives the
//! event to completion (or the frame budget), printing the host-call trace.

use std::process::ExitCode;

use mlua::{Table, Value};
use pla::script::{load_chunk, new_state};

/// Installs a metatable on _G so any unknown global becomes a recording stub.
const HOST_PROBE: &str = r#"
__host_calls = {}
local function make_stub(name)
  local stub
  stub = setmetatable({}, {
    __index = function(_, k) return make_stub(name .. "." .. tostring(k)) end,
    __call = function(_, ...) table.insert(__host_calls, name); return nil end,
    __tostring = function() return name end,
  })
  return stub
end
setmetatable(_G, {
  __index = function(t, k)
    local s = make_stub(tostring(k))
    rawset(t, k, s)
    return s
  end
})
"#;

fn describe(_lua: &mlua::Lua, v: &Value) -> String {
    match v {
        Value::Nil => "nil".into(),
        Value::Boolean(b) => format!("bool {b}"),
        Value::Integer(i) => format!("int {i}"),
        Value::Number(n) => format!("num {n}"),
        Value::String(s) => format!("str {:?}", s.to_string_lossy()),
        Value::Table(t) => {
            let mut keys: Vec<String> = Vec::new();
            for (k, val) in t.pairs::<Value, Value>().flatten() {
                let ks = match &k {
                    Value::String(s) => s.to_string_lossy().to_string(),
                    other => format!("{other:?}"),
                };
                keys.push(format!(
                    "{ks}={}",
                    match val {
                        Value::Table(_) => "table",
                        Value::Function(_) => "fn",
                        Value::String(_) => "str",
                        Value::Integer(_) => "int",
                        Value::Number(_) => "num",
                        Value::Boolean(_) => "bool",
                        _ => "other",
                    }
                ));
            }
            keys.sort();
            format!("table [{}]", keys.join(", "))
        }
        Value::Function(_) => "function".into(),
        Value::Thread(_) => "thread".into(),
        other => format!("{other:?}"),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: run_script <file.blua> [--probe]");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let probe = args.iter().any(|a| a == "--probe");
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return ExitCode::from(1);
        }
    };
    let lua = new_state();
    if args.iter().any(|a| a == "--host" || a == "--run") {
        pla::host::install(&lua).expect("host install");
    }
    if probe {
        lua.load(HOST_PROBE).exec().expect("probe setup");
    }
    if args.iter().any(|a| a == "--pcall-log") {
        lua.load(
            r#"
local real_pcall = pcall
pcall = function(f, ...)
  local ok, err = real_pcall(f, ...)
  if not ok then print("PCALL-ERR: " .. tostring(err)) end
  return ok, err
end
"#,
        )
        .exec()
        .expect("pcall log setup");
    }
    if args.iter().any(|a| a == "--compile-only") {
        match load_chunk(&lua, &bytes) {
            Ok(_) => println!("COMPILE ok"),
            Err(e) => println!("COMPILE failed: {e}"),
        }
        return ExitCode::SUCCESS;
    }
    let module: Table = match load_chunk(&lua, &bytes).and_then(|f| f.call::<Table>(())) {
        Ok(t) => t,
        Err(e) => {
            println!("LOAD/EXEC failed: {e:?}");
            return ExitCode::from(1);
        }
    };
    println!("LOAD  ok ({} bytes)", bytes.len());
    if args.iter().any(|a| a == "--run") {
        let class = args
            .iter()
            .position(|a| a == "--class")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "BoutiqueMainEvent".to_string());
        let target = pla::host::make_event_target(&lua, 4242).expect("engine target");
        let frames: usize = args
            .iter()
            .position(|a| a == "--frames")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(256);
        let run =
            pla::event::run(&lua, &module, &class, "general", target, frames).expect("event run");
        println!(
            "event {class}: {:?} after {} resume(s), result {}",
            run.outcome,
            run.frames,
            describe(&lua, &run.result)
        );
        if let Ok(calls) = lua.globals().get::<Table>("__host_calls") {
            for (i, v) in calls.sequence_values::<String>().enumerate() {
                if let Ok(name) = v {
                    println!("{i:4} {name}");
                }
            }
        }
        return ExitCode::SUCCESS;
    }
    let system: Table = match module.get("system") {
        Ok(t) => t,
        Err(e) => {
            println!("no `system` table: {e}");
            return ExitCode::SUCCESS;
        }
    };
    let mut names: Vec<String> = Vec::new();
    for (k, v) in system.pairs::<Value, Value>().flatten() {
        if let Value::String(s) = &k {
            let name = s.to_string_lossy().to_string();
            let ks = match v {
                Value::Table(_) => "table",
                Value::Function(_) => "fn",
                _ => "other",
            };
            names.push(format!("{name} ({ks})"));
        }
    }
    names.sort();
    println!("system classes: {}", names.len());
    for n in names.iter().take(20) {
        println!("  {n}");
    }
    if let Ok(class) = system.get::<Table>("EventScriptMain") {
        if let Ok(entry) = class.get::<mlua::Function>("ScriptCall") {
            println!("calling system.EventScriptMain.ScriptCall() ...");
            // Try a few argument shapes to discover the call protocol.
            let target = lua.create_table().unwrap();
            let get_id = lua.create_function(|_, ()| Ok(4242i64)).unwrap();
            target.set("GetTargetID", get_id).unwrap();
            let before = lua
                .globals()
                .get::<Table>("__host_calls")
                .map(|t| t.len().unwrap_or(0))
                .unwrap_or(0);
            let r = entry.call::<Value>(("BoutiqueMainEvent", "general", target.clone()));
            let after = lua
                .globals()
                .get::<Table>("__host_calls")
                .map(|t| t.len().unwrap_or(0))
                .unwrap_or(0);
            println!(
                "ScriptCall(\"BoutiqueMainEvent\", \"general\", target) -> {:?}, host calls: {}",
                r.map(|v| describe(&lua, &v))
                    .unwrap_or_else(|e| format!("err {e}")),
                after - before
            );
            for arg in ["main", "Main", "Start", "Boot", "Update", "ScriptCall"] {
                let before = lua
                    .globals()
                    .get::<Table>("__host_calls")
                    .map(|t| t.len().unwrap_or(0))
                    .unwrap_or(0);
                let r = entry.call::<Value>(arg);
                let after = lua
                    .globals()
                    .get::<Table>("__host_calls")
                    .map(|t| t.len().unwrap_or(0))
                    .unwrap_or(0);
                println!(
                    "ScriptCall({arg:?}) -> {:?}, host calls: {}",
                    r.map(|v| describe(&lua, &v).to_string())
                        .unwrap_or_else(|e| format!("err {e}")),
                    after - before
                );
            }
            for n in [0i64, 1i64] {
                let before = lua
                    .globals()
                    .get::<Table>("__host_calls")
                    .map(|t| t.len().unwrap_or(0))
                    .unwrap_or(0);
                let r = entry.call::<Value>(n);
                let after = lua
                    .globals()
                    .get::<Table>("__host_calls")
                    .map(|t| t.len().unwrap_or(0))
                    .unwrap_or(0);
                println!(
                    "ScriptCall({n}) -> {:?}, host calls: {}",
                    r.map(|v| describe(&lua, &v).to_string())
                        .unwrap_or_else(|e| format!("err {e}")),
                    after - before
                );
            }
            match entry.call::<Value>(()) {
                Ok(Value::Thread(th)) => {
                    println!("ScriptCall returned a COROUTINE");
                    match th.resume::<mlua::MultiValue>(()) {
                        Ok(vals) => {
                            println!("first resume yielded {} value(s):", vals.len());
                            for v in vals.iter().take(8) {
                                println!("  {}", describe(&lua, v));
                            }
                        }
                        Err(e) => println!("first resume stopped: {e}"),
                    }
                }
                Ok(Value::Function(f)) => {
                    println!("ScriptCall returned a function; calling it ...");
                    match f.call::<Value>(()) {
                        Ok(v) => println!("  -> {}", describe(&lua, &v)),
                        Err(e) => println!("  stopped: {e}"),
                    }
                }
                Ok(v) => println!("ScriptCall returned {}", describe(&lua, &v)),
                Err(e) => println!("ScriptCall stopped: {e}"),
            }
        }
    }
    if args.iter().any(|a| a == "--trace") {
        if let Ok(calls) = lua.globals().get::<Table>("__host_calls") {
            for (i, v) in calls.sequence_values::<String>().enumerate() {
                if let Ok(name) = v {
                    println!("{i:4} {name}");
                }
            }
        }
        return ExitCode::SUCCESS;
    }
    if probe || args.iter().any(|a| a == "--host") {
        if let Ok(calls) = lua.globals().get::<Table>("__host_calls") {
            println!("host calls recorded: {}", calls.len().unwrap_or(0));
            let mut seen: Vec<String> = Vec::new();
            for name in calls.sequence_values::<String>().flatten() {
                if !seen.contains(&name) {
                    seen.push(name);
                }
            }
            println!("distinct host functions requested: {}", seen.len());
            for n in seen.iter().take(40) {
                println!("  {n}");
            }
        }
    }
    ExitCode::SUCCESS
}
