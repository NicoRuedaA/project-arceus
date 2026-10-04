// @category Export
// Finds every reference to a given address and decompiles the referencing
// functions. Usage: ExportXrefs.java <output-dir> <address-hex> [--dump-refs]
//
// Prints one line per reference (from -> to, type) and writes the decompiled C
// of each referencing function, named <FUN>_<entry>.c, so the caller can read
// what touches the address.

import java.io.File;
import java.io.PrintWriter;
import java.util.LinkedHashSet;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.ReferenceManager;

public class ExportXrefs extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) {
            println("usage: ExportXrefs <out-dir> <address-hex>");
            return;
        }
        File outDir = new File(args[0]);
        outDir.mkdirs();
        Address target = currentProgram.getAddressFactory().getDefaultAddressSpace()
            .getAddress(args[1]);
        if (target == null) {
            println("bad address " + args[1]);
            return;
        }

        ReferenceManager rm = currentProgram.getReferenceManager();
        ReferenceIterator it = rm.getReferencesTo(target);
        Set<Address> froms = new LinkedHashSet<>();
        int n = 0;
        while (it.hasNext()) {
            Reference r = it.next();
            Address from = r.getFromAddress();
            println("REF " + from + " -> " + target + " [" + r.getReferenceType() + "]");
            froms.add(from);
            n++;
        }
        println("XREFS " + n + " from " + froms.size() + " sites");

        FunctionManager fm = currentProgram.getFunctionManager();
        DecompInterface dec = new DecompInterface();
        dec.toggleCCode(true);
        dec.toggleSyntaxTree(false);
        dec.openProgram(currentProgram);

        Set<Function> funcs = new LinkedHashSet<>();
        for (Address from : froms) {
            Function f = fm.getFunctionContaining(from);
            if (f != null) {
                funcs.add(f);
            }
        }
        int ok = 0;
        for (Function f : funcs) {
            DecompileResults res = dec.decompileFunction(f, 120, monitor);
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                File out = new File(outDir, f.getName() + "_"
                    + f.getEntryPoint().toString().replace("0x", "") + ".c");
                try (PrintWriter pw = new PrintWriter(out, "UTF-8")) {
                    pw.println("// name: " + f.getName());
                    pw.println("// entry: " + f.getEntryPoint());
                    pw.println("// signature: " + f.getPrototypeString(true, false));
                    pw.println();
                    pw.println(res.getDecompiledFunction().getC());
                }
                ok++;
            }
        }
        println("EXPORT-XREFS functions=" + funcs.size() + " ok=" + ok + " -> " + outDir);
    }
}
