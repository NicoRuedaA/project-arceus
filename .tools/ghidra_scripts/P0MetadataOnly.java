// @category Export
// Metadata-only inventory for P0. These rows imply zero semantic coverage.
// Usage: P0MetadataOnly.java <absolute-new-output-directory>

import java.io.BufferedWriter;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Iterator;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.reloc.Relocation;
import ghidra.program.model.reloc.RelocationTable;
import ghidra.program.model.symbol.ExternalLocation;
import ghidra.program.model.symbol.ExternalLocationIterator;
import ghidra.program.model.symbol.ExternalManager;

public class P0MetadataOnly extends GhidraScript {

    private static String tsv(String value) {
        if (value == null) return "";
        return value.replace("\\", "\\\\")
                .replace("\t", "\\t")
                .replace("\r", "\\r")
                .replace("\n", "\\n");
    }

    private static String address(Object value) {
        return value == null ? "" : tsv(value.toString());
    }

    private static BufferedWriter writer(File directory, String name) throws Exception {
        return Files.newBufferedWriter(new File(directory, name).toPath(), StandardCharsets.UTF_8);
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) {
            throw new IllegalArgumentException("expected exactly one absolute output-directory argument");
        }

        File outputDirectory = new File(args[0]);
        if (!outputDirectory.isAbsolute()) {
            throw new IllegalArgumentException("output directory must be absolute");
        }
        if (outputDirectory.exists()) {
            throw new IllegalArgumentException("output directory must not already exist");
        }
        if (!outputDirectory.mkdir()) {
            throw new IllegalStateException("could not create the requested output directory");
        }

        Memory memory = currentProgram.getMemory();
        MemoryBlock[] blocks = memory.getBlocks();
        int functionCount = 0;
        int relocationCount = 0;
        int externalCount = 0;

        try (BufferedWriter out = writer(outputDirectory, "memory-blocks.tsv")) {
            out.write("name\tstart\tend\tsize\tread\twrite\texecute\tinitialized\n");
            for (MemoryBlock block : blocks) {
                out.write(tsv(block.getName()));
                out.write('\t'); out.write(address(block.getStart()));
                out.write('\t'); out.write(address(block.getEnd()));
                out.write('\t'); out.write(Long.toString(block.getSize()));
                out.write('\t'); out.write(Boolean.toString(block.isRead()));
                out.write('\t'); out.write(Boolean.toString(block.isWrite()));
                out.write('\t'); out.write(Boolean.toString(block.isExecute()));
                out.write('\t'); out.write(Boolean.toString(block.isInitialized()));
                out.write('\n');
            }
        }

        FunctionManager functionManager = currentProgram.getFunctionManager();
        try (BufferedWriter out = writer(outputDirectory, "functions.tsv")) {
            out.write("entry\tname\tbody_bytes\texternal\tthunk\n");
            Iterator<Function> functions = functionManager.getFunctions(true);
            while (functions.hasNext()) {
                Function function = functions.next();
                out.write(address(function.getEntryPoint()));
                out.write('\t'); out.write(tsv(function.getName()));
                out.write('\t'); out.write(Long.toString(function.getBody().getNumAddresses()));
                out.write('\t'); out.write(Boolean.toString(function.isExternal()));
                out.write('\t'); out.write(Boolean.toString(function.isThunk()));
                out.write('\n');
                functionCount++;
            }
        }

        RelocationTable relocationTable = currentProgram.getRelocationTable();
        try (BufferedWriter out = writer(outputDirectory, "relocations.tsv")) {
            out.write("address\ttype\tstatus\n");
            Iterator<Relocation> relocations = relocationTable.getRelocations();
            while (relocations.hasNext()) {
                Relocation relocation = relocations.next();
                out.write(address(relocation.getAddress()));
                out.write('\t'); out.write(Integer.toString(relocation.getType()));
                out.write('\t'); out.write(tsv(relocation.getStatus().toString()));
                out.write('\n');
                relocationCount++;
            }
        }

        ExternalManager externalManager = currentProgram.getExternalManager();
        try (BufferedWriter out = writer(outputDirectory, "externals.tsv")) {
            out.write("library\tlabel\taddress\n");
            for (String libraryName : externalManager.getExternalLibraryNames()) {
                ExternalLocationIterator locations = externalManager.getExternalLocations(libraryName);
                while (locations.hasNext()) {
                    ExternalLocation location = locations.next();
                    out.write(tsv(location.getLibraryName()));
                    out.write('\t'); out.write(tsv(location.getLabel()));
                    out.write('\t'); out.write(address(location.getAddress()));
                    out.write('\n');
                    externalCount++;
                }
            }
        }

        try (BufferedWriter out = writer(outputDirectory, "program.tsv")) {
            out.write("name\tlanguage\tcompiler\timage_base\timage_min\timage_max\tmemory_blocks\tfunctions\trelocations\texternals\n");
            out.write(tsv(currentProgram.getName()));
            out.write('\t'); out.write(tsv(currentProgram.getLanguageID().toString()));
            out.write('\t'); out.write(tsv(currentProgram.getCompilerSpec().getCompilerSpecID().toString()));
            out.write('\t'); out.write(address(currentProgram.getImageBase()));
            out.write('\t'); out.write(address(currentProgram.getMinAddress()));
            out.write('\t'); out.write(address(currentProgram.getMaxAddress()));
            out.write('\t'); out.write(Integer.toString(blocks.length));
            out.write('\t'); out.write(Integer.toString(functionCount));
            out.write('\t'); out.write(Integer.toString(relocationCount));
            out.write('\t'); out.write(Integer.toString(externalCount));
            out.write('\n');
        }

        println("P0 metadata-only inventory complete; semantic coverage is not implied.");
    }
}
