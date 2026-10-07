import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;
import java.io.*;
import java.nio.charset.StandardCharsets;
public class ExportInstructionRange extends GhidraScript {
    public void run() throws Exception {
        String[] a = getScriptArgs();
        Address start = toAddr(a[1]), end = toAddr(a[2]);
        try (PrintWriter out = new PrintWriter(new File(a[0]), StandardCharsets.UTF_8)) {
            Instruction i = getInstructionAt(start);
            while (i != null && i.getAddress().compareTo(end) <= 0) {
                out.println(i.getAddress() + " " + i);
                i = getInstructionAfter(i.getAddress());
            }
        }
    }
}
