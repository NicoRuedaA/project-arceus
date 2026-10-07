// @category Analysis
// Read-only D1 audit: for every function flagged no-return, count RET instructions in its body
// and the call sites that precede a gap range. A function that really never returns normally has
// no reachable RET; one that has RET and many callers is a candidate wrong flag.
// Usage: NoReturnAudit.java <gap-prev.tsv> <output.tsv>. Emits addresses and counts only.

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.PrintWriter;
import java.util.HashMap;
import java.util.Map;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class NoReturnAudit extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("expected input and output paths");
        Map<String, Integer> gapAfter = new HashMap<>(); // call target -> gap ranges following a call to it
        try (BufferedReader in = new BufferedReader(new FileReader(args[0]))) {
            in.readLine();
            String line;
            while ((line = in.readLine()) != null) {
                String[] f = line.split("\t");
                if (!f[4].equals("-") && f[5].equals("true")) gapAfter.merge(f[4], 1, Integer::sum);
            }
        }
        Listing listing = currentProgram.getListing();
        try (PrintWriter out = new PrintWriter(args[1])) {
            out.println("entry\tsize\tis_thunk\tret_count\tbody_instructions\tcallers\tgap_ranges_after_call");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                monitor.checkCancelled();
                if (!f.hasNoReturn()) continue;
                int rets = 0, n = 0;
                for (Instruction ins : listing.getInstructions(f.getBody(), true)) {
                    n++;
                    if (ins.getFlowType().isTerminal() && ins.getMnemonicString().equals("ret")) rets++;
                }
                int callers = 0;
                ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(f.getEntryPoint());
                while (it.hasNext()) { Reference r = it.next(); if (r.getReferenceType().isCall()) callers++; }
                out.println(f.getEntryPoint() + "\t" + f.getBody().getNumAddresses() + "\t" + f.isThunk() + "\t" + rets + "\t" + n + "\t" + callers
                    + "\t" + gapAfter.getOrDefault(f.getEntryPoint().toString(), 0));
            }
        }
    }
}
