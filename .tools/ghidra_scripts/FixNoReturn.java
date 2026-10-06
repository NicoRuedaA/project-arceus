// @category Analysis
// Clears the non-returning flag of functions that return, then restores the
// fall-through after every call to them and recomputes the bodies of their callers.
// Without arguments a function returns when its body contains a RET. With a file,
// the listed entry points (one 8-hex address per line) are the returning functions,
// e.g. import stubs whose imported symbol returns (see REPRODUCE.md).
// Run on a writable copy of a project.
// Usage: FixNoReturn.java [returning-entries-file]

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.util.HashSet;
import java.util.Set;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.FlowOverride;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;

public class FixNoReturn extends GhidraScript {
    @Override
    public void run() throws Exception {
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        Set<Function> cleared = new HashSet<>();
        int noReturn = 0;
        Set<Address> listed = null;
        String[] args = getScriptArgs();
        if (args.length > 0) {
            listed = new HashSet<>();
            try (BufferedReader br = new BufferedReader(new FileReader(new File(args[0])))) {
                String line;
                while ((line = br.readLine()) != null) {
                    line = line.trim();
                    if (!line.isEmpty() && !line.startsWith("#")) {
                        listed.add(currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(line));
                    }
                }
            }
        }
        for (Function f : fm.getFunctions(true)) {
            if (!f.hasNoReturn()) continue;
            noReturn++;
            if (listed != null) {
                if (listed.contains(f.getEntryPoint())) cleared.add(f);
                continue;
            }
            for (Instruction i : listing.getInstructions(f.getBody(), true)) {
                if ("ret".equalsIgnoreCase(i.getMnemonicString())) {
                    cleared.add(f);
                    break;
                }
            }
        }
        for (Function f : cleared) f.setNoReturn(false);

        Set<Function> callers = new HashSet<>();
        int callSites = 0;
        for (Function f : cleared) {
            for (Reference ref : currentProgram.getReferenceManager().getReferencesTo(f.getEntryPoint())) {
                if (!ref.getReferenceType().isCall()) continue;
                Instruction call = listing.getInstructionAt(ref.getFromAddress());
                if (call == null) continue;
                callSites++;
                if (call.getFlowOverride() == FlowOverride.CALL_RETURN) call.setFlowOverride(FlowOverride.NONE);
                call.clearFallThroughOverride();
                Address next = call.getMaxAddress().next();
                if (next != null && listing.getInstructionAt(next) == null) {
                    new DisassembleCommand(next, null, true).applyTo(currentProgram, monitor);
                }
                Function caller = fm.getFunctionContaining(call.getAddress());
                if (caller != null) callers.add(caller);
            }
        }
        int fixed = 0;
        for (Function c : callers) {
            monitor.checkCancelled();
            if (CreateFunctionCmd.fixupFunctionBody(currentProgram, c, monitor)) fixed++;
        }
        println("FIX-NORETURN noreturn=" + noReturn + " cleared=" + cleared.size() + " call_sites=" + callSites
                + " callers=" + callers.size() + " bodies_fixed=" + fixed);
    }
}
