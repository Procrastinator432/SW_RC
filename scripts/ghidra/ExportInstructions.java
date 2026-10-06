import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportInstructions extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("Output and function addresses required");
        try (PrintWriter out = new PrintWriter(new File(args[0]), StandardCharsets.UTF_8)) {
            for (int i = 1; i < args.length; i++) {
                Function f = getFunctionAt(toAddr(args[i]));
                if (f == null) throw new IOException("Missing function " + args[i]);
                out.println("# " + f.getEntryPoint() + " " + f.getName(true));
                InstructionIterator instructions = currentProgram.getListing().getInstructions(f.getBody(), true);
                while (instructions.hasNext()) {
                    Instruction instruction = instructions.next();
                    out.println(instruction.getAddress() + " " + instruction);
                }
            }
        }
    }
}
