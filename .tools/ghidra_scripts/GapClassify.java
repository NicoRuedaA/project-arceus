// @category Analysis
// Read-only D1 query: classify every executable byte range outside all function bodies.
// Usage: GapClassify.java <output.tsv>. Writes per-interval metadata (addresses, sizes, classes);
// no bytes, instruction text or strings are emitted. Does not mutate the program.

import java.io.PrintWriter;
import java.util.Map;
import java.util.TreeMap;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressRange;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class GapClassify extends GhidraScript {
    private final Map<String, long[]> agg = new TreeMap<>(); // class -> {ranges, bytes}

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("expected output path");
        Memory mem = currentProgram.getMemory();
        AddressSet exec = new AddressSet();
        for (MemoryBlock b : mem.getBlocks()) if (b.isExecute() && b.isInitialized()) exec.add(b.getStart(), b.getEnd());
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        AddressSet bodies = new AddressSet();
        for (Function f : fm.getFunctions(true)) bodies.add(f.getBody());
        AddressSet gaps = exec.subtract(bodies);

        try (PrintWriter out = new PrintWriter(args[0])) {
            out.println("start\tlength\tclass\tblock\tinsn_bytes\tdata_bytes\tundef_bytes\tcall_in\tjump_in\tdata_in_fn\tdata_in_other\tpointer_units\tprev_fn_end_gap\tnext_fn_start_gap");
            for (AddressRange r : gaps.getAddressRanges()) {
                monitor.checkCancelled();
                long len = r.getLength();
                Address min = r.getMinAddress(), max = r.getMaxAddress();
                long insn = 0, data = 0, undef = 0, ptr = 0;
                Address p = min;
                while (p.compareTo(max) <= 0) {
                    CodeUnit cu = listing.getCodeUnitContaining(p);
                    if (cu == null) { undef++; p = p.add(1); continue; }
                    Address end = cu.getMaxAddress().compareTo(max) < 0 ? cu.getMaxAddress() : max;
                    long n = end.subtract(p) + 1;
                    if (cu instanceof Instruction) insn += n;
                    else if (cu instanceof Data) { data += n; if (((Data) cu).isPointer()) ptr++; }
                    else undef += n;
                    p = end.add(1);
                }
                // uniform content: every byte equal, or one repeated 4-byte word (padding fill)
                byte[] bytes = new byte[(int) Math.min(len, 1 << 20)];
                mem.getBytes(min, bytes);
                boolean uniform = true;
                boolean zero = true;
                for (byte b : bytes) if (b != 0) { zero = false; break; }
                if (!zero && len % 4 == 0 && bytes.length >= 4) {
                    for (int i = 4; i < bytes.length; i++) if (bytes[i] != bytes[i - 4]) { uniform = false; break; }
                } else uniform = zero;

                long callIn = 0, jumpIn = 0, dataFn = 0, dataOther = 0;
                for (Address a = min; a.compareTo(max) <= 0; ) {
                    ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(a);
                    while (it.hasNext()) {
                        Reference ref = it.next();
                        Address from = ref.getFromAddress();
                        if (gaps.contains(from) && from.compareTo(min) >= 0 && from.compareTo(max) <= 0) continue; // self-reference
                        if (ref.getReferenceType().isCall()) callIn++;
                        else if (ref.getReferenceType().isJump()) jumpIn++;
                        else if (fm.getFunctionContaining(from) != null) dataFn++;
                        else dataOther++;
                    }
                    // walk unit starts only for speed
                    CodeUnit cu = listing.getCodeUnitContaining(a);
                    if (cu == null) a = a.add(1);
                    else { Address e = cu.getMaxAddress(); if (e.compareTo(max) >= 0) break; a = e.add(1); }
                }

                String cls;
                if (zero) cls = "PAD_ZERO";
                else if (uniform) cls = "PAD_REPEATED_WORD";
                else if (callIn + jumpIn > 0 && insn > 0) cls = "CODE_FLOW_REFERENCED";
                else if (insn > 0 && data == 0 && undef == 0) cls = "CODE_DEFINED_UNREFERENCED";
                else if (insn > 0) cls = "MIXED_CODE_DATA";
                else if (dataFn > 0) cls = "DATA_REF_FROM_FUNCTION";
                else if (dataOther > 0) cls = "DATA_REF_FROM_OTHER";
                else if (data > 0 && undef == 0) cls = "DATA_UNREFERENCED";
                else cls = "UNDEFINED_OR_OTHER";

                Address before = min.subtract(1), after = max.add(1);
                boolean prevFn = fm.getFunctionContaining(before) != null;
                boolean nextFn = fm.getFunctionContaining(after) != null;
                MemoryBlock blk = mem.getBlock(min);
                out.println(min + "\t" + len + "\t" + cls + "\t" + (blk == null ? "-" : blk.getName()) + "\t" + insn + "\t" + data + "\t" + undef
                    + "\t" + callIn + "\t" + jumpIn + "\t" + dataFn + "\t" + dataOther + "\t" + ptr + "\t" + prevFn + "\t" + nextFn);
                long[] t = agg.computeIfAbsent(cls, k -> new long[2]);
                t[0]++; t[1] += len;
            }
        }
        long rs = 0, bs = 0;
        for (Map.Entry<String, long[]> e : agg.entrySet()) {
            println("CLASS " + e.getKey() + " ranges=" + e.getValue()[0] + " bytes=" + e.getValue()[1]);
            rs += e.getValue()[0]; bs += e.getValue()[1];
        }
        println("TOTAL ranges=" + rs + " bytes=" + bs);
        println("FUNCTIONS " + fm.getFunctionCount() + " BODY_BYTES " + bodies.getNumAddresses() + " EXEC_BYTES " + exec.getNumAddresses());
    }
}
