import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
public class ExportIntroGoals extends GhidraScript {
    public void run() throws Exception {
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        try (PrintWriter out = new PrintWriter(new File(getScriptArgs()[0]), StandardCharsets.UTF_8)) {
            FunctionIterator fs = currentProgram.getFunctionManager().getFunctions(true);
            while (fs.hasNext()) {
                Function f = fs.next(); String n = f.getName(true);
                if (!(n.startsWith("UGOAL_Goto::") || n.startsWith("UGOAL_WaitForLanding::"))) continue;
                if (n.contains("UGOAL_Goto::UGOAL_Goto") || n.contains("~") || f.isThunk()) continue;
                out.println("/* " + f.getEntryPoint() + " " + n + " */");
                DecompileResults r = d.decompileFunction(f,60,monitor);
                if (r.decompileCompleted()) out.println(r.getDecompiledFunction().getC());
            }
        } finally { d.dispose(); }
    }
}
