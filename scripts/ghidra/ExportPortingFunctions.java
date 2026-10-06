// Run after Ghidra analysis. Pseudocode is evidence, not compilable recovered source.
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.listing.Function;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportPortingFunctions extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length != 1) throw new IllegalArgumentException("Output directory required");
        File dir = new File(args[0], currentProgram.getName());
        if (!dir.isDirectory() && !dir.mkdirs()) throw new IOException("Cannot create " + dir);
        DecompInterface decompiler = new DecompInterface();
        if (!decompiler.openProgram(currentProgram)) throw new IOException("Cannot open decompiler");
        int count = 0;
        try (PrintWriter index = new PrintWriter(new File(dir,"functions.tsv"), StandardCharsets.UTF_8);
             PrintWriter code = new PrintWriter(new File(dir,"porting-functions.c"), StandardCharsets.UTF_8)) {
            index.println("address\tname\tselected\tdecompiled");
            code.println("/* Ghidra pseudocode; original binary: " + currentProgram.getName() + ". Not buildable source. */");
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                monitor.checkCancelled();
                String name = f.getName();
                String key = name.toLowerCase();
                boolean selected = !f.isExternal() && !f.isThunk() && (key.contains("tick") || key.contains("player") || key.contains("pawn") || key.contains("weapon") || key.contains("commando") || key.contains("move") || key.contains("input") || key.contains("init") || key.contains("main"));
                boolean success = false;
                if (selected && count < 150) {
                    DecompileResults result = decompiler.decompileFunction(f, 30, monitor);
                    success = result.decompileCompleted() && result.getDecompiledFunction() != null;
                    if (success) {
                        code.println("\n/* " + f.getEntryPoint() + " " + name + " */");
                        code.println(result.getDecompiledFunction().getC());
                        count++;
                    } else code.println("/* Failed: " + f.getEntryPoint() + " " + result.getErrorMessage() + " */");
                }
                index.println(f.getEntryPoint() + "\t" + name.replace('\t',' ') + "\t" + selected + "\t" + success);
            }
        } finally { decompiler.dispose(); }
        println("Exported " + count + " functions to " + dir);
    }
}
