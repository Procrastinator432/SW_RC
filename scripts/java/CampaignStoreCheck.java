package org.rcport.diagnostic;
import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.zip.*;

public final class CampaignStoreCheck {
    interface Action {void run()throws IOException;}
    static void reject(Action action)throws IOException{try{action.run();throw new AssertionError("Invalid package accepted");}catch(IOException expected){}}
    static byte[] pack(Map<String,byte[]> scenes,String mode)throws Exception{
        StringBuilder manifest=new StringBuilder("format=RCCAMPAIGN1\n");
        for(String map:CampaignStore.MAPS){byte[] b=scenes.get(map);StringBuilder sha=new StringBuilder();for(byte x:MessageDigest.getInstance("SHA-256").digest(b))sha.append(String.format("%02x",x&255));manifest.append(map).append(".file=").append(map.toLowerCase(Locale.ROOT)).append(".rcplay\n").append(map).append(".bytes=").append(b.length).append('\n').append(map).append(".sha256=").append(sha).append('\n');}
        if(mode.equals("duplicate"))manifest.append("format=RCCAMPAIGN1\n");
        ByteArrayOutputStream out=new ByteArrayOutputStream();
        try(ZipOutputStream zip=new ZipOutputStream(out)){
            zip.putNextEntry(new ZipEntry("campaign.properties"));zip.write(manifest.toString().getBytes(java.nio.charset.StandardCharsets.US_ASCII));zip.closeEntry();
            for(String map:CampaignStore.MAPS){if(mode.equals("missing")&&map.equals("GEO_03A"))continue;byte[] b=scenes.get(map).clone();if(mode.equals("corrupt")&&map.equals("GEO_01B"))b[9]^=1;zip.putNextEntry(new ZipEntry(map.toLowerCase(Locale.ROOT)+".rcplay"));zip.write(b);zip.closeEntry();}
            if(mode.equals("path")){zip.putNextEntry(new ZipEntry("../escape"));zip.write(1);zip.closeEntry();}
        }return out.toByteArray();
    }
    public static void main(String[] args)throws Exception{
        File base=new File(args[0]);base.mkdirs();File dir=Files.createTempDirectory(base.toPath(),"campaign-").toFile();CampaignStore store=new CampaignStore(dir);
        if(store.ready())throw new AssertionError();Map<String,byte[]> scenes=new LinkedHashMap<>();for(String map:CampaignStore.MAPS)scenes.put(map,("RCPLAY14scene-"+map).getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        CampaignStore.Validator validator=(file,map)->{if(!new String(Files.readAllBytes(file.toPath()),java.nio.charset.StandardCharsets.US_ASCII).endsWith(map))throw new IOException("Wrong native map");};
        byte[] valid=pack(scenes,"");store.importPackage(new ByteArrayInputStream(valid),validator,s->{});if(!store.ready())throw new AssertionError();
        for(String map:CampaignStore.MAPS)if(!Arrays.equals(scenes.get(map),Files.readAllBytes(store.scene(map).toPath())))throw new AssertionError();
        File index=new File(dir,"campaign/package.properties");byte[] before=Files.readAllBytes(index.toPath());
        for(String mode:new String[]{"duplicate","missing","corrupt","path"}){
            byte[] bad=pack(scenes,mode);reject(()->store.importPackage(new ByteArrayInputStream(bad),validator,s->{}));
            if(!Arrays.equals(before,Files.readAllBytes(index.toPath())) || !store.ready() || new File(dir,"campaign/staging").list().length!=0)throw new AssertionError("Rejected import changed installed package: "+mode);
        }
        reject(()->store.importPackage(new ByteArrayInputStream(valid),(f,m)->{throw new IOException("Native validation failed");},s->{}));
        if(!Arrays.equals(before,Files.readAllBytes(index.toPath())))throw new AssertionError();
        reject(()->store.scene("../../escape"));
        File a=store.scene("GEO_01A");String key=a.getName();byte[] checkpoint=new byte[20];new CheckpointStore(dir).save(key,"GEO_01A",checkpoint);
        byte[] saved=Files.readAllBytes(new File(dir,"checkpoints/resume.save").toPath());Files.write(a.toPath(),new byte[]{1});reject(()->store.scene("GEO_01A"));if(store.ready())throw new AssertionError();
        Files.write(new File(dir,"campaign/staging/map-interrupted.tmp").toPath(),new byte[]{1});
        store.importPackage(new ByteArrayInputStream(valid),validator,s->{});store.importPackage(new ByteArrayInputStream(valid),validator,s->{});
        if(!store.ready() || new File(dir,"checkpoints/scenes").list().length!=4 || !Arrays.equals(saved,Files.readAllBytes(new File(dir,"checkpoints/resume.save").toPath())))throw new AssertionError("Repair changed saves or duplicated scenes");
        System.out.println("{\"passed\":true,\"four_maps_imported\":true,\"corruption_missing_duplicate_path_rejected\":true,\"native_rejection_atomic\":true,\"reimport_repairs_cache_preserves_checkpoint\":true,\"interrupted_staging_cleaned\":true,\"no_scene_duplication\":true}");
    }
}
