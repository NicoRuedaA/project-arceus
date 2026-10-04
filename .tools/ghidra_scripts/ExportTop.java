// @category Export
// Exports decompiled C for a bounded list of functions (Mode B triage workflow).
// Usage: ExportTop.java <output-dir> <addresses-file>
// The addresses file has one function entry-point address per line (0x... hex).

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.HashSet;
import java.util.Set;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;

public class ExportTop extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args[0]);
        File listFile = new File(args[1]);
        outDir.mkdirs();

        Set<String> wanted = new HashSet<>();
        try (BufferedReader br = new BufferedReader(new FileReader(listFile))) {
            String line;
            while ((line = br.readLine()) != null) {
                line = line.trim();
                if (line.isEmpty() || line.startsWith("#")) continue;
                wanted.add(line.toLowerCase());
            }
        }

        DecompInterface dec = new DecompInterface();
        dec.toggleCCode(true);
        dec.toggleSyntaxTree(false);
        dec.openProgram(currentProgram);

        FunctionManager fm = currentProgram.getFunctionManager();
        int ok = 0;
        int fail = 0;
        int missing = 0;
        for (String addrStr : wanted) {
            Address addr = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(addrStr);
            Function f = fm.getFunctionAt(addr);
            if (f == null) {
                missing++;
                continue;
            }
            DecompileResults res = dec.decompileFunction(f, 120, monitor);
            if (res != null && res.decompileCompleted() && res.getDecompiledFunction() != null) {
                File out = new File(outDir, f.getName() + "_" + addrStr.replace("0x", "") + ".c");
                try (PrintWriter pw = new PrintWriter(out, "UTF-8")) {
                    pw.println("// name: " + f.getName());
                    pw.println("// entry: " + f.getEntryPoint());
                    pw.println("// signature: " + f.getPrototypeString(true, false));
                    pw.println(res.getDecompiledFunction().getC());
                }
                ok++;
            } else {
                fail++;
            }
        }
        println("EXPORT-TOP wanted=" + wanted.size() + " ok=" + ok + " fail=" + fail + " missing=" + missing
                + " -> " + outDir);
        dec.dispose();
    }
}
