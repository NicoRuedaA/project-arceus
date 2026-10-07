// @category Analysis
// Read-only helper: print the body size of each function entry listed (one 8-hex address per line).
// Usage: FuncSizes.java <entries.txt> <output.tsv>
import java.io.BufferedReader;
import java.io.FileReader;
import java.io.PrintWriter;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class FuncSizes extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        try (BufferedReader in = new BufferedReader(new FileReader(args[0])); PrintWriter out = new PrintWriter(args[1])) {
            out.println("entry\tsize");
            String line;
            while ((line = in.readLine()) != null) {
                line = line.trim();
                if (line.isEmpty()) continue;
                Address a = currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress(Long.parseLong(line, 16));
                Function f = currentProgram.getFunctionManager().getFunctionAt(a);
                out.println(line + "\t" + (f == null ? -1 : f.getBody().getNumAddresses()));
            }
        }
    }
}
