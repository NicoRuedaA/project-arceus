// @category Analysis
// Read-only D1 query: what ends the function body immediately before each gap range?
// Usage: GapPrev.java <gap-probe.tsv> <output.tsv>. Emits addresses, mnemonics classes and flags only.

import java.io.BufferedReader;
import java.io.FileReader;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.Listing;

public class GapPrev extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 2) throw new IllegalArgumentException("expected input and output paths");
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        try (BufferedReader in = new BufferedReader(new FileReader(args[0])); PrintWriter out = new PrintWriter(args[1])) {
            in.readLine();
            out.println("start\tprev_function\tprev_last_mnemonic\tprev_last_flow\tcall_target\ttarget_noreturn\ttarget_thunk\tgap_first_flow_into_prev_fn");
            String line;
            while ((line = in.readLine()) != null) {
                monitor.checkCancelled();
                String[] f = line.split("\t");
                Address min = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(Long.parseLong(f[0], 16));
                Function prev = fm.getFunctionContaining(min.subtract(1));
                String mn = "-", flow = "-", tgt = "-", nr = "-", th = "-";
                if (prev != null) {
                    Instruction last = listing.getInstructionContaining(min.subtract(1));
                    if (last != null) {
                        mn = last.getMnemonicString();
                        flow = last.getFlowType().getName();
                        if (last.getFlowType().isCall() && last.getFlows().length == 1) {
                            Address t = last.getFlows()[0];
                            tgt = t.toString();
                            Function tf = fm.getFunctionAt(t);
                            if (tf != null) { nr = String.valueOf(tf.hasNoReturn()); th = String.valueOf(tf.isThunk()); }
                        }
                    }
                }
                out.println(f[0] + "\t" + (prev == null ? "-" : prev.getEntryPoint().toString()) + "\t" + mn + "\t" + flow + "\t" + tgt + "\t" + nr + "\t" + th + "\t-");
            }
        }
    }
}
