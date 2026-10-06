// @category Analysis
// Creates functions at a list of candidate entry points (see .tools/function_seeds.py).
// A seed already inside a function is skipped. Run on a writable copy of a project.
// Usage: SeedFunctions.java <seeds-file>
// The seeds file has one entry-point address per line (8-hex).

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.util.ArrayList;
import java.util.List;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.FunctionManager;

public class SeedFunctions extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        List<String> seeds = new ArrayList<>();
        try (BufferedReader br = new BufferedReader(new FileReader(new File(args[0])))) {
            String line;
            while ((line = br.readLine()) != null) {
                line = line.trim();
                if (!line.isEmpty() && !line.startsWith("#")) seeds.add(line);
            }
        }
        FunctionManager fm = currentProgram.getFunctionManager();
        int created = 0, inside = 0, noCode = 0, failed = 0;
        for (String s : seeds) {
            monitor.checkCancelled();
            Address addr = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(s);
            if (fm.getFunctionContaining(addr) != null) {
                inside++;
                continue;
            }
            if (currentProgram.getListing().getInstructionAt(addr) == null) {
                new DisassembleCommand(addr, null, true).applyTo(currentProgram, monitor);
                if (currentProgram.getListing().getInstructionAt(addr) == null) {
                    noCode++;
                    continue;
                }
            }
            CreateFunctionCmd cmd = new CreateFunctionCmd(addr);
            if (cmd.applyTo(currentProgram, monitor) && fm.getFunctionAt(addr) != null) {
                created++;
            } else {
                failed++;
            }
        }
        println("SEED-FUNCTIONS seeds=" + seeds.size() + " created=" + created + " inside_existing=" + inside
                + " no_code=" + noCode + " failed=" + failed + " total_functions=" + fm.getFunctionCount());
    }
}
