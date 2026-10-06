import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.Function;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportAddresses extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("Output file and addresses required");
        DecompInterface d = new DecompInterface();
        if (!d.openProgram(currentProgram)) throw new IOException("Cannot open decompiler");
        try (PrintWriter out = new PrintWriter(new File(args[0]), StandardCharsets.UTF_8)) {
            for (int i=1;i<args.length;i++) {
                Function f = getFunctionAt(toAddr(args[i]));
                if (f == null) { out.println("/* No function at " + args[i] + " */"); continue; }
                DecompileResults r = d.decompileFunction(f,60,monitor);
                out.println("/* " + f.getEntryPoint() + " " + f.getName(true) + " */");
                if (r.decompileCompleted()) out.println(r.getDecompiledFunction().getC());
                else out.println("/* " + r.getErrorMessage() + " */");
            }
        } finally { d.dispose(); }
    }
}
