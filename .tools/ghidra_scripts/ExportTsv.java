// @category Export
// Exports Mode B evidence sheets from the analyzed program:
//   functions.tsv  id  name  size  calls  called_by  tags  status
//   strings.tsv    id  addr  text  x_refs  use
//   callgraph.tsv  from  to  kind
//   structs.tsv    id  kind  size  fields  status
//   fields.tsv     struct  name  offset  type  note
// Usage: ExportTsv.java <output-dir>

import java.io.File;
import java.io.PrintWriter;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.data.Composite;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeComponent;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.Structure;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.FunctionTag;
import ghidra.program.util.DefinedDataIterator;

public class ExportTsv extends GhidraScript {

    private PrintWriter open(File dir, String name) throws Exception {
        return new PrintWriter(new File(dir, name), "UTF-8");
    }

    private static String clean(String s) {
        if (s == null) return "";
        return s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ');
    }

    /** Doctrine id charset: ^[a-z0-9_]+$. */
    private static String slug(String s) {
        return s.toLowerCase().replaceAll("[^a-z0-9_]", "_");
    }

    private static String tags(Function f) {
        List<String> names = new ArrayList<>();
        for (FunctionTag t : f.getTags()) {
            names.add(t.getName());
        }
        return String.join(" ", names);
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File dir = new File(args.length > 0 ? args[0] : ".");
        dir.mkdirs();

        FunctionManager fm = currentProgram.getFunctionManager();

        // functions.tsv
        PrintWriter fn = open(dir, "functions.tsv");
        fn.println("# sheet: re/functions");
        fn.println("# version: 1");
        fn.println("# generator: ExportTsv.java (ghidra)");
        fn.println("# target: re");
        fn.println("# requires: -");
        fn.println("# doctrine: mode-b");
        fn.println("id:string*\tname:string\tsize:u32\tcalls:u32\tcalled_by:u32\ttags:string?\tstatus:enum");
        int fnCount = 0;
        for (Function f : fm.getFunctions(true)) {
            int calls = f.getCalledFunctions(monitor).size();
            int calledBy = f.getCallingFunctions(monitor).size();
            fn.println(clean(f.getEntryPoint().toString()) + "\t" + clean(f.getName()) + "\t"
                    + f.getBody().getNumAddresses() + "\t" + calls + "\t" + calledBy + "\t"
                    + clean(tags(f)) + "\tidentified");
            fnCount++;
        }
        fn.close();

        // strings.tsv
        PrintWriter st = open(dir, "strings.tsv");
        st.println("# sheet: re/strings");
        st.println("# version: 1");
        st.println("# generator: ExportTsv.java (ghidra)");
        st.println("# target: re");
        st.println("# requires: -");
        st.println("# doctrine: mode-b");
        st.println("id:string*\taddr:string\ttext:string\tx_refs:u32\tuse:enum");
        int stCount = 0;
        for (Data d : DefinedDataIterator.byDataType(currentProgram,
                dt -> dt.getName().toLowerCase().contains("string") || dt.getName().toLowerCase().contains("char"))) {
            String text = d.getDefaultValueRepresentation();
            if (text == null || text.length() < 4) continue;
            int xrefs = currentProgram.getReferenceManager().getReferenceCountTo(d.getAddress());
            st.println(slug(d.getAddress().toString()) + "\t" + clean(d.getAddress().toString()) + "\t"
                    + clean(text) + "\t" + xrefs + "\tunknown");
            stCount++;
        }
        st.close();

        // callgraph.tsv
        PrintWriter cg = open(dir, "callgraph.tsv");
        cg.println("# sheet: re/callgraph");
        cg.println("# version: 1");
        cg.println("# generator: ExportTsv.java (ghidra)");
        cg.println("# target: re");
        cg.println("# requires: functions");
        cg.println("# doctrine: mode-b");
        cg.println("from:string\tto:string\tkind:enum");
        int cgCount = 0;
        for (Function f : fm.getFunctions(true)) {
            for (Function callee : f.getCalledFunctions(monitor)) {
                cg.println(clean(f.getEntryPoint().toString()) + "\t" + clean(callee.getEntryPoint().toString()) + "\tcall");
                cgCount++;
            }
        }
        cg.close();

        // structs.tsv + fields.tsv
        PrintWriter sd = open(dir, "structs.tsv");
        sd.println("# sheet: re/structs");
        sd.println("# version: 1");
        sd.println("# generator: ExportTsv.java (ghidra)");
        sd.println("# target: re");
        sd.println("# requires: -");
        sd.println("# doctrine: mode-b");
        sd.println("id:string*\tname:string\tkind:string\tsize:u32\tfields:u32\tstatus:enum");
        PrintWriter fl = open(dir, "fields.tsv");
        fl.println("# sheet: re/fields");
        fl.println("# version: 1");
        fl.println("# generator: ExportTsv.java (ghidra)");
        fl.println("# target: re");
        fl.println("# requires: structs");
        fl.println("# doctrine: mode-b");
        fl.println("struct:string -> re/structs.id\tname:string\toffset:u32\ttype:string\tnote:string?");
        int sdCount = 0;
        int flCount = 0;
        DataTypeManager dtm = currentProgram.getDataTypeManager();
        Iterator<DataType> dtIt = dtm.getAllDataTypes();
        while (dtIt.hasNext()) {
            DataType dt = dtIt.next();
            if (!(dt instanceof Composite)) continue;
            Composite c = (Composite) dt;
            sd.println(slug(c.getName()) + "\t" + clean(c.getName()) + "\t" + dt.getClass().getSimpleName() + "\t" + c.getLength()
                    + "\t" + c.getNumComponents() + "\tidentified");
            sdCount++;
            if (dt instanceof Structure) {
                for (DataTypeComponent comp : c.getComponents()) {
                    fl.println(slug(c.getName()) + "\t" + clean(comp.getFieldName()) + "\t" + comp.getOffset()
                            + "\t" + clean(comp.getDataType().getName()) + "\t");
                    flCount++;
                }
            }
        }
        sd.close();
        fl.close();

        println("EXPORT-TSV functions=" + fnCount + " strings=" + stCount + " callgraph=" + cgCount
                + " structs=" + sdCount + " fields=" + flCount + " -> " + dir);
        if (fnCount == 0) {
            throw new RuntimeException("functions.tsv is empty; export failed");
        }
    }
}
