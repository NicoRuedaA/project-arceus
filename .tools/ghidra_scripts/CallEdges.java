// @category Analysis
// Read-only helper: dump caller->callee function edges (calls and jump/tail-call references).
// Usage: CallEdges.java <output.tsv>
import java.io.PrintWriter;
import java.util.HashSet;
import java.util.Set;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class CallEdges extends GhidraScript {
    @Override
    public void run() throws Exception {
        FunctionManager fm = currentProgram.getFunctionManager();
        long n = 0;
        try (PrintWriter out = new PrintWriter(getScriptArgs()[0])) {
            out.println("caller\tcallee\tkind");
            Set<String> seen = new HashSet<>();
            for (Function callee : fm.getFunctions(true)) {
                monitor.checkCancelled();
                ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(callee.getEntryPoint());
                while (it.hasNext()) {
                    Reference r = it.next();
                    String kind = r.getReferenceType().isCall() ? "call" : r.getReferenceType().isJump() ? "jump" : null;
                    if (kind == null) continue;
                    Function caller = fm.getFunctionContaining(r.getFromAddress());
                    if (caller == null || caller.getEntryPoint().equals(callee.getEntryPoint())) continue;
                    String key = caller.getEntryPoint() + ">" + callee.getEntryPoint();
                    if (!seen.add(key)) continue;
                    out.println(caller.getEntryPoint() + "\t" + callee.getEntryPoint() + "\t" + kind);
                    n++;
                }
            }
        }
        println("EDGES " + n);
    }
}
