// @category Analysis
// Exact coverage of the executable memory by function bodies (read-only).
// Prints executable bytes, union of function bodies, instruction bytes outside
// functions and the number of functions marked non-returning.
// Usage: CoverageStats.java

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.mem.MemoryBlock;

public class CoverageStats extends GhidraScript {
    @Override
    public void run() throws Exception {
        AddressSet exec = new AddressSet();
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            if (b.isExecute() && b.isInitialized()) exec.add(b.getStart(), b.getEnd());
        }
        AddressSet bodies = new AddressSet();
        int noReturn = 0;
        int functions = 0;
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
            bodies.add(f.getBody());
            functions++;
            if (f.hasNoReturn()) noReturn++;
        }
        AddressSet inExec = bodies.intersect(exec);
        long instrOutside = 0;
        long instrTotal = 0;
        for (Instruction i : currentProgram.getListing().getInstructions(exec, true)) {
            instrTotal += i.getLength();
            if (!bodies.contains(i.getAddress())) instrOutside += i.getLength();
        }
        println("COVERAGE functions=" + functions + " exec_bytes=" + exec.getNumAddresses()
                + " body_union_bytes=" + inExec.getNumAddresses()
                + " instruction_bytes=" + instrTotal
                + " instruction_bytes_outside_functions=" + instrOutside
                + " noreturn_functions=" + noReturn);
    }
}
