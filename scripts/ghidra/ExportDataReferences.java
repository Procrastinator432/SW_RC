import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportDataReferences extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        try (PrintWriter out = new PrintWriter(new File(args[0]), StandardCharsets.UTF_8)) {
            for (int i = 1; i < args.length; i++) {
                out.println("TARGET " + args[i]);
                for (Reference ref : getReferencesTo(toAddr(args[i]))) {
                    Function f = getFunctionContaining(ref.getFromAddress());
                    out.println(ref.getFromAddress() + " " + ref.getReferenceType() + " " +
                        (f == null ? "outside function" : f.getEntryPoint() + " " + f.getName(true)));
                }
            }
        }
    }
}
