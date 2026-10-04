// @category Export
// Exports function -> string references, for subsystem clustering by path.
//   strrefs.tsv  string_addr  function_addr  function_name  text
// Usage: ExportStrRefs.java <output-dir>

import java.io.File;
import java.io.PrintWriter;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.util.DefinedDataIterator;

public class ExportStrRefs extends GhidraScript {

    private static String clean(String s) {
        if (s == null) return "";
        return s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File dir = new File(args.length > 0 ? args[0] : ".");
        dir.mkdirs();

        PrintWriter out = new PrintWriter(new File(dir, "strrefs.tsv"), "UTF-8");
        out.println("# sheet: re/strrefs");
        out.println("# version: 1");
        out.println("# generator: ExportStrRefs.java (ghidra)");
        out.println("# target: re");
        out.println("# requires: re/strings, re/functions");
        out.println("# doctrine: mode-b");
        out.println("string_addr\tfunction_addr\tfunction_name\ttext");

        int strings = 0;
        int refs = 0;
        for (Data d : DefinedDataIterator.byDataType(currentProgram,
                dt -> dt.getName().toLowerCase().contains("string") || dt.getName().toLowerCase().contains("char"))) {
            String text = d.getDefaultValueRepresentation();
            if (text == null || text.length() < 4) continue;
            strings++;
            for (Reference r : getReferencesTo(d.getAddress())) {
                Function f = getFunctionContaining(r.getFromAddress());
                if (f == null) continue;
                out.println(clean(d.getAddress().toString()) + "\t" + clean(f.getEntryPoint().toString()) + "\t"
                        + clean(f.getName()) + "\t" + clean(text));
                refs++;
            }
        }
        out.close();
        println("EXPORT-STRREFS strings=" + strings + " refs=" + refs + " -> " + dir);
    }
}
