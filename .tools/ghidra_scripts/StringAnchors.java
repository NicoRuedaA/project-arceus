// @category Analysis
// Read-only D2 query: for every defined string, list the functions that reference it.
// Usage: StringAnchors.java <output.tsv>. Output stays in work/ (it contains game/library strings).
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.data.StringDataInstance;

public class StringAnchors extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        long strings = 0, refs = 0;
        try (PrintWriter out = new PrintWriter(args[0])) {
            out.println("function\tstring_addr\tvalue");
            DataIterator it = currentProgram.getListing().getDefinedData(true);
            while (it.hasNext()) {
                monitor.checkCancelled();
                Data d = it.next();
                StringDataInstance sdi = StringDataInstance.getStringDataInstance(d);
                if (sdi == StringDataInstance.NULL_INSTANCE) continue;
                String v = sdi.getStringValue();
                if (v == null || v.length() < 3) continue;
                strings++;
                ReferenceIterator ri = currentProgram.getReferenceManager().getReferencesTo(d.getAddress());
                while (ri.hasNext()) {
                    Reference r = ri.next();
                    Function f = currentProgram.getFunctionManager().getFunctionContaining(r.getFromAddress());
                    if (f == null) continue;
                    refs++;
                    out.println(f.getEntryPoint() + "\t" + d.getAddress() + "\t" + v.replace('\t', ' ').replace('\n', ' ').replace('\r', ' '));
                }
            }
        }
        println("STRINGS " + strings + " FUNCTION_REFS " + refs);
    }
}
