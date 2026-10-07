// @category Analysis
// Read-only D1 probe over the non-padding gap ranges listed by GapClassify.
// Usage: GapProbe.java <gap-classes.tsv> <output.tsv>. Each range is trial-disassembled inside a
// transaction that is always rolled back, so the program is never changed. Emits only counts/ratios.

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.PrintWriter;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.mem.Memory;

public class GapProbe extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("expected input and output paths");
        Memory mem = currentProgram.getMemory();
        Listing listing = currentProgram.getListing();
        AddressSet image = new AddressSet(mem);
        try (BufferedReader in = new BufferedReader(new FileReader(args[0])); PrintWriter out = new PrintWriter(args[1])) {
            in.readLine();
            out.println("start\tlength\tclass\tascii_ratio\tptr_word_ratio\tzero_word_ratio\tdecoded_bytes\tdecoded_ratio\tfirst_fail_offset\tret_count\tbranch_to_outside\tprologue_at_start\taligned4\tcall_total\tcall_to_fn_entry\tcall_to_other_in_image\tlast_is_terminal\taligned16");
            String line;
            while ((line = in.readLine()) != null) {
                monitor.checkCancelled();
                String[] f = line.split("\t");
                String cls = f[2];
                if (cls.startsWith("PAD_")) continue;
                Address min = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(Long.parseLong(f[0], 16));
                long len = Long.parseLong(f[1]);
                Address max = min.add(len - 1);
                int n = (int) Math.min(len, 1 << 20);
                byte[] b = new byte[n];
                mem.getBytes(min, b);
                long printable = 0, zeroWords = 0, ptrWords = 0, words = 0;
                for (byte x : b) if ((x >= 0x20 && x < 0x7f) || x == 0x0a || x == 0x09) printable++;
                for (int i = 0; i + 8 <= n; i += 8) {
                    long v = 0;
                    for (int k = 7; k >= 0; k--) v = (v << 8) | (b[i + k] & 0xffL);
                    words++;
                    if (v == 0) zeroWords++;
                    else if (v > 0 && v < 0x06000000L && mem.contains(min.getNewAddress(v))) ptrWords++;
                }
                boolean aligned = (min.getOffset() & 3) == 0 && (len & 3) == 0;
                long decoded = 0, firstFail = -1, rets = 0, outside = 0, callTotal = 0, callEntry = 0, callOther = 0;
                boolean lastTerminal = false;
                boolean prologue = false;
                int tx = currentProgram.startTransaction("probe");
                try {
                    listing.clearCodeUnits(min, max, false);
                    if (aligned) {
                        DisassembleCommand cmd = new DisassembleCommand(min, new AddressSet(min, max), true);
                        cmd.setSeedContext(null);
                        cmd.applyTo(currentProgram, monitor);
                        for (Instruction ins : listing.getInstructions(new AddressSet(min, max), true)) {
                            decoded += ins.getLength();
                            String m = ins.getMnemonicString();
                            if (m.equals("ret")) rets++;
                            if (ins.getFlowType().isCall() && m.equals("bl")) {
                                for (Address t : ins.getFlows()) {
                                    callTotal++;
                                    if (currentProgram.getFunctionManager().getFunctionAt(t) != null) callEntry++;
                                    else if (image.contains(t)) callOther++;
                                }
                            }
                            if (ins.getFlowType().isJump() || ins.getFlowType().isCall()) {
                                for (Address t : ins.getFlows()) if (!(t.compareTo(min) >= 0 && t.compareTo(max) <= 0) && !image.contains(t)) outside++;
                            }
                        }
                        Instruction lastIns = listing.getInstructionBefore(max.add(1));
                        lastTerminal = lastIns != null && lastIns.getFlowType().isTerminal();
                        Instruction first = listing.getInstructionAt(min);
                        if (first != null) {
                            String s = first.toString();
                            prologue = s.startsWith("stp x29,x30,[sp, #-") || s.startsWith("sub sp,sp,") || s.startsWith("stp x2");
                        }
                        // first byte offset not covered by a decoded instruction
                        for (long off = 0; off < len; off += 4) {
                            if (listing.getInstructionAt(min.add(off)) == null) { firstFail = off; break; }
                        }
                    }
                } finally {
                    currentProgram.endTransaction(tx, false); // always roll back
                }
                out.println(f[0] + "\t" + len + "\t" + cls + "\t" + String.format("%.3f", printable / (double) n) + "\t"
                    + String.format("%.3f", words == 0 ? 0 : ptrWords / (double) words) + "\t" + String.format("%.3f", words == 0 ? 0 : zeroWords / (double) words)
                    + "\t" + decoded + "\t" + String.format("%.3f", decoded / (double) len) + "\t" + firstFail + "\t" + rets + "\t" + outside + "\t" + prologue + "\t" + aligned
                    + "\t" + callTotal + "\t" + callEntry + "\t" + callOther + "\t" + lastTerminal + "\t" + ((min.getOffset() & 15) == 0));
            }
        }
    }
}
