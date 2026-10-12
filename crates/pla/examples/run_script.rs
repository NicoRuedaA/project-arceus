//! Run a Haxe-compiled Lua 5.3 event script and probe its host API needs.
//!
//! Usage: cargo run -p pla --example run_script -- <file.blua> [--run] [--module <name>] [--class <name>] [--probe]
//!
//! `--run` installs the host layer, builds the engine target and drives the
//! event to completion (or the frame budget), printing the host-call trace.

use std::process::ExitCode;
use std::{
    fs::File,
    io::{BufWriter, Write},
};

use mlua::{Table, Value};
use pla::script::{load_chunk, new_state};

/// Installs a metatable on _G so any unknown global becomes a recording stub.
const HOST_PROBE: &str = r#"
    __host_calls = {}
    __host_call_count = 0
    __host_call_dropped = 0
    __host_call_limit = 2048
    __record_host_call = function(name)
      __host_call_count = __host_call_count + 1
      name = string.sub(tostring(name), 1, 256)
      if #__host_calls < __host_call_limit then
        table.insert(__host_calls, name)
      else
        __host_call_dropped = __host_call_dropped + 1
      end
    end
local function make_stub(name)
  local stub
  stub = setmetatable({}, {
    __index = function(_, k) return make_stub(name .. "." .. tostring(k)) end,
    __call = function(_, ...)
      __record_host_call(name)
      return nil
    end,
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

fn write_trace(lua: &mlua::Lua, path: Option<&String>, max_lines: usize) -> Result<(), String> {
    let calls = lua
        .globals()
        .get::<Table>("__host_calls")
        .map_err(|e| format!("read host trace: {e}"))?;
    let count = usize::try_from(calls.len().unwrap_or(0))
        .unwrap_or(0)
        .min(max_lines);
    if let Some(path) = path {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create trace directory {}: {e}", parent.display()))?;
        }
        let file = File::create(path).map_err(|e| format!("create trace {path}: {e}"))?;
        let mut out = BufWriter::new(file);
        for value in calls.sequence_values::<String>().take(count) {
            writeln!(
                out,
                "{}",
                value.map_err(|e| format!("decode host trace: {e}"))?
            )
            .map_err(|e| format!("write trace {path}: {e}"))?;
        }
        out.flush()
            .map_err(|e| format!("flush trace {path}: {e}"))?;
    } else {
        for (i, value) in calls.sequence_values::<String>().take(count).enumerate() {
            println!(
                "{i:4} {}",
                value.map_err(|e| format!("decode host trace: {e}"))?
            );
        }
    }
    let total = lua
        .globals()
        .get::<u64>("__host_call_count")
        .unwrap_or(count as u64);
    let dropped = lua.globals().get::<u64>("__host_call_dropped").unwrap_or(0);
    println!("TRACE lines={count} total={total} dropped={dropped} cap={max_lines}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "usage: run_script <file.blua> [--run] [--module <name>] [--class <name>] [--probe]"
        );
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
    if let Some(limit) = args
        .iter()
        .position(|a| a == "--lua-memory-limit-mib")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse::<usize>().ok())
    {
        let bytes = limit.saturating_mul(1024 * 1024);
        if let Err(e) = lua.set_memory_limit(bytes) {
            eprintln!("set Lua memory limit to {bytes} bytes: {e}");
            return ExitCode::from(1);
        }
    }
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
local first_error = nil
pcall = function(f, ...)
  local ok, err = real_pcall(f, ...)
  if not ok and first_error == nil then
    first_error = string.sub(tostring(err), 1, 512)
    print("PCALL-ERR: " .. first_error)
  end
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
        let module_name = args
            .iter()
            .position(|a| a == "--module")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "general".to_string());
        let target = pla::host::make_event_target(&lua, 4242).expect("engine target");
        let frames: usize = args
            .iter()
            .position(|a| a == "--frames")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(256);
        let run = pla::event::run(&lua, &module, &class, &module_name, target, frames)
            .expect("event run");
        println!(
            "event {module_name}.{class}: {:?} after {} resume(s), result {}",
            run.outcome,
            run.frames,
            describe(&lua, &run.result)
        );
        if args.iter().any(|a| a == "--trace" || a == "--trace-file") {
            let trace_file = args
                .iter()
                .position(|a| a == "--trace-file")
                .and_then(|i| args.get(i + 1));
            let max_lines = args
                .iter()
                .position(|a| a == "--trace-max-lines")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(2048);
            if let Err(e) = write_trace(&lua, trace_file, max_lines) {
                eprintln!("{e}");
                return ExitCode::from(1);
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
        let trace_file = args
            .iter()
            .position(|a| a == "--trace-file")
            .and_then(|i| args.get(i + 1));
        let max_lines = args
            .iter()
            .position(|a| a == "--trace-max-lines")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(2048);
        if let Err(e) = write_trace(&lua, trace_file, max_lines) {
            eprintln!("{e}");
            return ExitCode::from(1);
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
