import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

public class ExportIndirectCalls extends GhidraScript {
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 2) throw new IllegalArgumentException("Output and operand offsets required");
        try (PrintWriter out = new PrintWriter(new File(args[0]), StandardCharsets.UTF_8)) {
            InstructionIterator it = currentProgram.getListing().getInstructions(true);
            while (it.hasNext()) {
                Instruction ins = it.next();
                if (!ins.getMnemonicString().equals("CALL")) continue;
                String operand = ins.getDefaultOperandRepresentation(0);
                for (int i=1;i<args.length;i++) {
                    if (!operand.contains("+ " + args[i] + "]")) continue;
                    Function f = getFunctionContaining(ins.getAddress());
                    out.println(ins.getAddress()+" "+ins+" FUNCTION "+(f==null?"none":f.getEntryPoint()+" "+f.getName(true)));
                    break;
                }
            }
        }
    }
}
