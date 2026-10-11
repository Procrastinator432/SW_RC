import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
public class DefineFunctions extends GhidraScript {
    public void run() throws Exception {
        for (String value : getScriptArgs()) {
            Address address = toAddr(value);
            if (getFunctionAt(address) == null) {
                if (!disassemble(address)) throw new Exception("Cannot disassemble " + value);
                if (createFunction(address, null) == null) throw new Exception("Cannot define " + value);
            }
        }
    }
}
