// @category Export
// Exports the decompiled C of every function in the current program.
// Usage: ExportDecompiled.java <output-dir>

import java.io.File;
import java.io.PrintWriter;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

public class ExportDecompiled extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args.length > 0 ? args[0] : ".");
        outDir.mkdirs();

        DecompInterface dec = new DecompInterface();
        dec.toggleCCode(true);
        dec.toggleSyntaxTree(false);
        dec.openProgram(currentProgram);

        FunctionManager fm = currentProgram.getFunctionManager();
        int total = 0;
        int ok = 0;
        int fail = 0;
        for (Function f : fm.getFunctions(true)) {
            total++;
            if (f.isThunk() || f.isExternal()) {
                continue;
            }
            DecompileResults res = dec.decompileFunction(f, 60, monitor);
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                String c = res.getDecompiledFunction().getC();
                File out = new File(outDir, f.getName() + ".c");
                try (PrintWriter pw = new PrintWriter(out, "UTF-8")) {
                    pw.println("// name: " + f.getName());
                    pw.println("// entry: " + f.getEntryPoint());
                    pw.println("// signature: " + f.getPrototypeString(true, false));
                    pw.println(c);
                }
                ok++;
            } else {
                fail++;
            }
            if (total % 5000 == 0) {
                println("progress: " + total + " functions, ok=" + ok + " fail=" + fail);
            }
        }
        println("EXPORT DONE total=" + total + " ok=" + ok + " fail=" + fail);
        dec.dispose();
    }
}
