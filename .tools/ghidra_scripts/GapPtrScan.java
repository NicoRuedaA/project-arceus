// @category Analysis
// Read-only D1 query: count 8-byte aligned little-endian values in every initialized non-executable
// block that equal the start of a gap range (or an existing function entry, as a control).
// Usage: GapPtrScan.java <gap-probe.tsv> <output.tsv>. Emits addresses and counts only.

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.HashMap;
import java.util.Map;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;

public class GapPtrScan extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("expected input and output paths");
        Memory mem = currentProgram.getMemory();
        Map<Long, long[]> starts = new HashMap<>(); // address -> {count in data}
        try (BufferedReader in = new BufferedReader(new FileReader(args[0]))) {
            in.readLine();
            String line;
            while ((line = in.readLine()) != null) starts.put(Long.parseLong(line.split("\t")[0], 16), new long[1]);
        }
        Map<Long, long[]> entries = new HashMap<>();
        for (Function f : currentProgram.getFunctionManager().getFunctions(true)) entries.put(f.getEntryPoint().getOffset(), new long[1]);
        long scanned = 0;
        StringBuilder blocks = new StringBuilder();
        for (MemoryBlock b : mem.getBlocks()) {
            if (!b.isInitialized() || b.isExecute()) continue;
            blocks.append(b.getName()).append('=').append(b.getSize()).append(' ');
            byte[] buf = new byte[(int) Math.min(b.getSize(), 1 << 26)];
            mem.getBytes(b.getStart(), buf);
            for (int i = 0; i + 8 <= buf.length; i += 8) {
                long v = 0;
                for (int k = 7; k >= 0; k--) v = (v << 8) | (buf[i + k] & 0xffL);
                long[] c = starts.get(v);
                if (c != null) c[0]++;
                long[] e = entries.get(v);
                if (e != null) e[0]++;
                scanned++;
            }
        }
        long gapHit = 0, entryHit = 0;
        for (long[] c : starts.values()) if (c[0] > 0) gapHit++;
        for (long[] c : entries.values()) if (c[0] > 0) entryHit++;
        println("BLOCKS " + blocks);
        println("SCANNED_QWORDS " + scanned);
        println("GAP_STARTS " + starts.size() + " pointed_to_by_data " + gapHit);
        println("FUNCTION_ENTRIES " + entries.size() + " pointed_to_by_data " + entryHit);
        try (PrintWriter out = new PrintWriter(args[1])) {
            out.println("start\tdata_pointers");
            for (Map.Entry<Long, long[]> e : starts.entrySet()) out.println(String.format("%08x", e.getKey()) + "\t" + e.getValue()[0]);
        }
    }
}
