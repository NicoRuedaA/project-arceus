// @category Analysis
// Recovers switch jump tables in functions that hold an unresolved computed jump
// (BR without computed references). Each such function is decompiled and the
// decompiler's jump tables are applied with DecompilerSwitchAnalysisCmd, the same
// command Ghidra's Decompiler Switch Analysis uses: case targets are read from the
// table, disassembled, and the owning function body is recomputed.
// Processes functions in chunks and stops starting new chunks after the time cap.
// Run on a writable copy of a project.
// Usage: SwitchTables.java [timeout-seconds-per-function] [total-cap-minutes]

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

import ghidra.app.cmd.function.DecompilerSwitchAnalysisCmd;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.decompiler.parallel.DecompilerCallback;
import ghidra.app.decompiler.parallel.ParallelDecompiler;
import ghidra.app.plugin.core.analysis.SwitchAnalysisDecompileConfigurer;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.symbol.FlowType;
import ghidra.program.model.symbol.Reference;
import ghidra.util.task.TaskMonitor;

public class SwitchTables extends GhidraScript {
    private int unresolvedSites(Set<Function> out) {
        FunctionManager fm = currentProgram.getFunctionManager();
        int sites = 0;
        for (Instruction i : currentProgram.getListing().getInstructions(true)) {
            FlowType ft = i.getFlowType();
            if (!ft.isJump() || !ft.isComputed()) continue;
            boolean resolved = false;
            for (Reference r : i.getReferencesFrom()) {
                if (r.getReferenceType().isComputed()) {
                    resolved = true;
                    break;
                }
            }
            if (resolved) continue;
            Function f = fm.getFunctionContaining(i.getAddress());
            if (f == null || f.isThunk()) continue;
            sites++;
            if (out != null) out.add(f);
        }
        return sites;
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        int timeout = args.length > 0 ? Integer.parseInt(args[0]) : 60;
        long capMs = (args.length > 1 ? Long.parseLong(args[1]) : 50) * 60_000L;
        long start = System.currentTimeMillis();

        Set<Function> todo = new LinkedHashSet<>();
        int sitesBefore = unresolvedSites(todo);
        println("SWITCH-TABLES unresolved_sites=" + sitesBefore + " functions=" + todo.size());

        DecompilerCallback<Void> callback = new DecompilerCallback<Void>(currentProgram,
                new SwitchAnalysisDecompileConfigurer(currentProgram)) {
            @Override
            public Void process(DecompileResults results, TaskMonitor m) throws Exception {
                new DecompilerSwitchAnalysisCmd(results).applyTo(currentProgram, m);
                return null;
            }
        };
        callback.setTimeout(timeout);
        List<Function> all = new ArrayList<>(todo);
        int done = 0;
        boolean capped = false;
        try {
            for (int i = 0; i < all.size(); i += 2000) {
                if (System.currentTimeMillis() - start > capMs) {
                    capped = true;
                    break;
                }
                List<Function> chunk = all.subList(i, Math.min(all.size(), i + 2000));
                ParallelDecompiler.decompileFunctions(callback, chunk, monitor);
                done += chunk.size();
            }
        }
        finally {
            callback.dispose();
        }
        int sitesAfter = unresolvedSites(null);
        println("SWITCH-TABLES processed_functions=" + done + " of " + all.size() + " capped=" + capped
                + " unresolved_sites_after=" + sitesAfter + " resolved_sites=" + (sitesBefore - sitesAfter)
                + " minutes=" + (System.currentTimeMillis() - start) / 60000);
    }
}
