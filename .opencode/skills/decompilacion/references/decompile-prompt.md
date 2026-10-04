# Decompile Prompt (Ghidra-first)

Reference for the decompile phase. Load before running any decompiler.

## 1. Identify the target directory

- Ask for, or locate, the **parent folder of the game** to decompile.
- Both the decompiler and `gamedb` search that folder **recursively**.
- Example (Windows): `C:\Users\Lab\Documents\Decomps\Fish game decomp`
- Linux/macOS equivalent: the same folder, e.g. `/mnt/dev/decomps/fish-game`
- Decompiled output goes to a sibling `decompiled/` folder:
  `.../Fish game decomp/decompiled`
- Record the exact absolute path before touching anything. Do not guess it.

## 2. Run this prompt (Ghidra as the default tool)

```
Decompile the game binary within the directory I've provided using Ghidra as the
default tool — but first identify the binary's format/language (check magic bytes,
PE/ELF headers, section/segment names, imported runtimes, symbol tables, and
strings: e.g. native C/C++, .NET/IL, Java bytecode, Delphi/Pascal, Lua bytecode,
PyInstaller-packed Python, Unity IL2CPP metadata, Unreal script containers).

Identify the program — report the file path, size, architecture, bitness, and the
evidence that determined its language/bytecode format.

Pick the tool — use Ghidra for native code. If the format is managed or scripted,
grab and use a purpose-built decompiler instead (.NET → ILSpy/dnSpy/ILSpyCmd,
Java → JADX/CFR/Procyon, Lua → unluac/LuaDec, Python bytecode →
decompyle3/uncompyle6/pycdc, Unity IL2CPP → Il2CppDumper + Cpp2IL,
Delphi → IDR/Delphi decompiler plugins). Justify the choice in one line.

Produce the output — import/load the binary, run auto-analysis to completion, then
export decompiled source for all relevant classes/functions to a clean output
directory with sensible file names.

Handle packers/obfuscation — if the binary is packed or protected, identify the
protector and note what's needed to reach the real code (and unpack/handle it if
feasible).

Summarize — list the top-level modules/classes, the main entry point and game
loop, and anything notable (anti-debug, license checks, network calls, encryption
of game data), with file paths and addresses for each finding.

Report progress as you go, and flag anything that blocks progress instead of
guessing.
```

## 3. Point the next phase at the output

`gamedb` indexes the **`decompiled/` output folder** (`-r .../decompiled`), never the
game folder. Then follow `gamedb-indexing.md`.
