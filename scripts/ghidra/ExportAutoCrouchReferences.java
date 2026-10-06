import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportAutoCrouchReferences extends GhidraScript {
    public void run() throws Exception {
        try (PrintWriter out = new PrintWriter(new File(getScriptArgs()[0]), StandardCharsets.UTF_8)) {
            Function f = getFunctionAt(toAddr(getScriptArgs().length > 1 ? getScriptArgs()[1] : "1048db00"));
            for (Reference ref : getReferencesTo(f.getEntryPoint())) {
                Function caller = getFunctionContaining(ref.getFromAddress());
                out.println("REFERENCE " + ref.getFromAddress() + " " + ref.getReferenceType() + " " + (caller == null ? "outside function" : caller.getEntryPoint() + " " + caller.getName(true)));
            }
            InstructionIterator instructions = currentProgram.getListing().getInstructions(f.getBody(), true);
            while (instructions.hasNext()) {
                Instruction i = instructions.next();
                out.println(i.getAddress() + " " + i);
            }
        }
    }
}
