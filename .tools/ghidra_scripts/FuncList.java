// @category Analysis
// Read-only helper: dump every function entry with body size, thunk flag and caller count.
// Usage: FuncList.java <output.tsv>
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;

public class FuncList extends GhidraScript {
    @Override
    public void run() throws Exception {
        try (PrintWriter out = new PrintWriter(getScriptArgs()[0])) {
            out.println("entry\tsize\tis_thunk\tcallers");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                int callers = 0;
                ReferenceIterator it = currentProgram.getReferenceManager().getReferencesTo(f.getEntryPoint());
                while (it.hasNext()) { Reference r = it.next(); if (r.getReferenceType().isCall() || r.getReferenceType().isJump()) callers++; }
                out.println(f.getEntryPoint() + "\t" + f.getBody().getNumAddresses() + "\t" + f.isThunk() + "\t" + callers);
            }
        }
    }
}
