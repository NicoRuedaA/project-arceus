// @category Analysis
// Finds executable gaps outside every function body and classifies each gap's
// first non-padding address by what precedes it. In "create" mode, creates a
// function at gap starts that follow a terminator (ret, b, br, a call to a
// non-returning function) or padding: a heuristic, recorded separately from
// reference-backed entries. Repeats until no function is created.
// Usage: GapSeeds.java <stats|create> [created-list-file]

import java.io.File;
import java.io.PrintWriter;
import java.util.Map;
import java.util.TreeMap;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressRange;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;

public class GapSeeds extends GhidraScript {
    private static boolean isPad(int w) {
        return w == 0 || w == 0xE7FFDEFE || w == 0xD503201F;
    }

    private String prevKind(Address a, Memory mem, Listing listing, FunctionManager fm) throws Exception {
        Address p = a.subtract(4);
        if (isPad(mem.getInt(p))) return "padding";
        Instruction i = listing.getInstructionAt(p);
        if (i == null) return "undisassembled";
        String m = i.getMnemonicString().toLowerCase();
        if (m.equals("ret")) return "ret";
        if (m.equals("br")) return "br";
        if (m.equals("b")) return "b";
        if (i.getFlowType().isCall()) {
            Address[] flows = i.getFlows();
            if (flows.length == 1) {
                Function callee = fm.getFunctionAt(flows[0]);
                if (callee != null && callee.hasNoReturn()) return "call_noreturn";
            }
            return "call_returning";
        }
        return "other";
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        boolean create = args.length > 0 && args[0].equals("create");
        PrintWriter out = args.length > 1 ? new PrintWriter(new File(args[1]), "UTF-8") : null;
        Memory mem = currentProgram.getMemory();
        Listing listing = currentProgram.getListing();
        FunctionManager fm = currentProgram.getFunctionManager();
        AddressSet exec = new AddressSet();
        for (MemoryBlock b : mem.getBlocks()) {
            if (b.isExecute() && b.isInitialized()) exec.add(b.getStart(), b.getEnd());
        }
        int round = 0;
        int totalCreated = 0;
        while (true) {
            round++;
            AddressSet bodies = new AddressSet();
            for (Function f : fm.getFunctions(true)) bodies.add(f.getBody());
            AddressSet gaps = exec.subtract(bodies);
            Map<String, long[]> kinds = new TreeMap<>();
            int created = 0;
            for (AddressRange r : gaps.getAddressRanges()) {
                monitor.checkCancelled();
                Address a = r.getMinAddress();
                while (a.compareTo(r.getMaxAddress()) < 0 && isPad(mem.getInt(a))) a = a.add(4);
                if (a.compareTo(r.getMaxAddress()) >= 0 || a.getOffset() == 0) continue;
                String k = prevKind(a, mem, listing, fm);
                long[] c = kinds.computeIfAbsent(k, x -> new long[2]);
                c[0]++;
                c[1] += r.getMaxAddress().subtract(a) + 1;
                boolean eligible = k.equals("ret") || k.equals("b") || k.equals("br")
                        || k.equals("call_noreturn") || k.equals("padding");
                if (!create || !eligible || fm.getFunctionContaining(a) != null) continue;
                if (listing.getInstructionAt(a) == null) {
                    new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
                    if (listing.getInstructionAt(a) == null) continue;
                }
                if (new CreateFunctionCmd(a).applyTo(currentProgram, monitor) && fm.getFunctionAt(a) != null) {
                    created++;
                    if (out != null) out.println(String.format("%08x\t%s", a.getOffset(), k));
                }
            }
            for (Map.Entry<String, long[]> e : kinds.entrySet()) {
                println("GAP-SEEDS round=" + round + " kind=" + e.getKey() + " gaps=" + e.getValue()[0]
                        + " bytes=" + e.getValue()[1]);
            }
            println("GAP-SEEDS round=" + round + " created=" + created);
            totalCreated += created;
            if (!create || created == 0 || round >= 20) break;
        }
        if (out != null) out.close();
        println("GAP-SEEDS total_created=" + totalCreated + " functions=" + fm.getFunctionCount());
    }
}
