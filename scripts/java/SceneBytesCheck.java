package org.rcport.diagnostic;
import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Run with a heap smaller than two scene payloads to detect accidental copying. */
public final class SceneBytesCheck {
    interface Read {void run() throws IOException;}
    static void rejected(Read read) throws IOException {
        try {read.run();throw new AssertionError("Malformed scene accepted");}
        catch(IOException expected) {}
    }
    public static void main(String[] args) throws Exception {
        File scene=new File(args[1]),cache=new File(args[2]);cache.mkdirs();
        rejected(() -> SceneBytes.read(new ByteArrayInputStream(new byte[2]),0,cache));
        rejected(() -> SceneBytes.read(new ByteArrayInputStream(new byte[2]),3,cache));
        rejected(() -> SceneBytes.read(new ByteArrayInputStream(new byte[2]),1,cache));
        rejected(() -> SceneBytes.read(new ByteArrayInputStream(new byte[0]),(long)SceneBytes.LIMIT+1,cache));
        rejected(() -> SceneBytes.read(new ByteArrayInputStream(new byte[0]),-1,cache));
        byte[] small={1,2,3,4};
        InputStream zero=new ByteArrayInputStream(small) {
            boolean once=true;
            @Override public synchronized int read(byte[] b,int at,int n) {
                if(once){once=false;return 0;}return super.read(b,at,n);
            }
        };
        if(!Arrays.equals(SceneBytes.read(zero,4,cache),small))throw new AssertionError();
        final byte[] payload;
        try(InputStream input=new FileInputStream(scene)) {
            payload=SceneBytes.read(input,args[0].equals("unknown")?-1:scene.length(),cache);
        }
        if(payload.length!=scene.length())throw new AssertionError();
        StringBuilder digest=new StringBuilder();
        for(byte b:MessageDigest.getInstance("SHA-256").digest(payload))digest.append(String.format("%02x",b&255));
        if(cache.list().length!=0)throw new AssertionError("Temporary scene not removed");
        System.out.println("{\"passed\":true,\"mode\":\""+args[0]+"\",\"bytes\":"+payload.length+",\"heap_limit\":"+Runtime.getRuntime().maxMemory()+",\"sha256\":\""+digest+"\",\"malformed_inputs_rejected\":true,\"temporary_files_removed\":true}");
    }
}
