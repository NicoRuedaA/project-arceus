// @category Export
// Exports the disassembly of a bounded list of functions, plus the decompiler
// result for each one. Fallback for functions the decompiler cannot handle.
// Usage: ExportAsm.java <output-dir> <addresses-file> [timeout-seconds]
// The addresses file has one function entry-point address per line (8-hex).
// timeout-seconds defaults to 120 per function.

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.List;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;

public class ExportAsm extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File outDir = new File(args[0]);
        File listFile = new File(args[1]);
        int timeout = args.length > 2 ? Integer.parseInt(args[2]) : 120;
        outDir.mkdirs();

        List<String> wanted = new ArrayList<>();
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

        int ok = 0;
        int missing = 0;
        for (String addrStr : wanted) {
            Address addr = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(addrStr);
            Function f = currentProgram.getFunctionManager().getFunctionAt(addr);
            if (f == null) {
                missing++;
                continue;
            }
            DecompileResults res = dec.decompileFunction(f, timeout, monitor);
            boolean decOk = res != null && res.decompileCompleted() && res.getDecompiledFunction() != null;
            String why = res == null ? "no result" : res.getErrorMessage();
            File out = new File(outDir, f.getName() + "_" + addrStr.replace("0x", "") + ".s");
            int count = 0;
            try (PrintWriter pw = new PrintWriter(out, "UTF-8")) {
                pw.println("// name: " + f.getName());
                pw.println("// entry: " + f.getEntryPoint());
                pw.println("// body: " + f.getBody().getMinAddress() + "-" + f.getBody().getMaxAddress()
                        + " (" + f.getBody().getNumAddresses() + " bytes)");
                pw.println("// decompiler: " + (decOk ? "ok" : "failed: " + (why == null ? "" : why.trim())));
                InstructionIterator it = currentProgram.getListing().getInstructions(f.getBody(), true);
                while (it.hasNext()) {
                    Instruction ins = it.next();
                    StringBuilder hex = new StringBuilder();
                    for (byte b : ins.getBytes()) hex.append(String.format("%02x", b));
                    pw.println(ins.getAddress() + "\t" + hex + "\t" + ins);
                    count++;
                }
            }
            println("EXPORT-ASM " + addrStr + " instructions=" + count + " decompiler=" + (decOk ? "ok" : "failed"));
            ok++;
        }
        println("EXPORT-ASM wanted=" + wanted.size() + " ok=" + ok + " missing=" + missing + " -> " + outDir);
        dec.dispose();
    }
}
