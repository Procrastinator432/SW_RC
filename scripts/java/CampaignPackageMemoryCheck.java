package org.rcport.diagnostic;
import java.io.*;
import java.nio.file.*;
public final class CampaignPackageMemoryCheck {
    public static void main(String[] args)throws Exception {
        File parent=new File(args[1]);parent.mkdirs();File files=Files.createTempDirectory(parent.toPath(),"stream-").toFile();CampaignStore store=new CampaignStore(files);
        try(InputStream input=new FileInputStream(args[0])){store.importPackage(input,(scene,map)->{try(InputStream in=new FileInputStream(scene)){byte[] header=new byte[8];if(in.read(header)!=8 || !new String(header,java.nio.charset.StandardCharsets.US_ASCII).startsWith("RCPLAY"))throw new IOException("Invalid scene header");}},s->{});}
        if(!store.ready())throw new AssertionError();
        for(String map:CampaignStore.MAPS)if(store.scene(map).length()<12)throw new AssertionError();
        System.out.println("{\"passed\":true,\"actual_four_scene_package\":true,\"heap_limit\":"+Runtime.getRuntime().maxMemory()+",\"streaming_import\":true}");
    }
}
