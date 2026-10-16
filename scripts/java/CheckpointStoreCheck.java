package org.rcport.diagnostic;
import java.io.*;
import java.nio.file.*;
import java.util.*;
public final class CheckpointStoreCheck {
    interface Read {void run() throws IOException;}
    static void reject(Read r)throws IOException {try {r.run();throw new AssertionError("Invalid save accepted");}catch(IOException expected){}}
    public static void main(String[] args)throws Exception {
        File base=new File(args[0]);base.mkdirs();File run=Files.createTempDirectory(base.toPath(),"store-").toFile();
        CheckpointStore store=new CheckpointStore(run);if(store.hasSave())throw new AssertionError();
        byte[] scene="RCPLAY14original-private-scene".getBytes(java.nio.charset.StandardCharsets.UTF_8);String name=store.cache(scene);
        byte[] a=new byte[40],b=new byte[41],c=new byte[42];a[0]=1;b[0]=2;c[0]=3;
        store.save(name,"first",a);store.save(name,"second",b);
        if(!Arrays.equals(store.load(false).state,b)||!Arrays.equals(store.load(true).state,a))throw new AssertionError();
        File root=new File(run,"checkpoints"),primary=new File(root,"resume.save");
        for(String blocked:new String[]{"resume.save.tmp","previous.save.tmp"}) {
            byte[] primaryBefore=Files.readAllBytes(primary.toPath()),backupBefore=Files.readAllBytes(new File(root,"previous.save").toPath());
            File obstruction=new File(root,blocked);if(!obstruction.mkdir())throw new AssertionError();
            File child=new File(obstruction,"keep");Files.write(child.toPath(),new byte[]{1});
            reject(() -> store.save(name,"write must fail",c));
            CheckpointStore reopened=new CheckpointStore(run);
            if(!Arrays.equals(primaryBefore,Files.readAllBytes(primary.toPath())) || !Arrays.equals(backupBefore,Files.readAllBytes(new File(root,"previous.save").toPath())) || !Arrays.equals(reopened.load(false).state,b) || !Arrays.equals(reopened.load(true).state,a))throw new AssertionError("Failed write changed committed generations: "+blocked);
            Files.delete(child.toPath());Files.delete(obstruction.toPath());
        }
        if(new File(root,"resume.save.tmp").exists())throw new AssertionError("Staged primary leaked after backup failure");
        store.save(name,"retry",c);store.save(name,"return to baseline",b);store.save(name,"first",a);store.save(name,"second",b);
        byte[] beforeDuplicate=Files.readAllBytes(primary.toPath()),previousDuplicate=Files.readAllBytes(new File(root,"previous.save").toPath());
        store.save(name,"same state",b);
        if(!Arrays.equals(beforeDuplicate,Files.readAllBytes(primary.toPath()))||!Arrays.equals(previousDuplicate,Files.readAllBytes(new File(root,"previous.save").toPath())))throw new AssertionError("Duplicate rotated the backup");
        byte[] corrupt=Files.readAllBytes(primary.toPath());corrupt[corrupt.length-1]^=1;Files.write(primary.toPath(),corrupt);
        reject(() -> store.load(false));if(!Arrays.equals(store.load(true).state,a))throw new AssertionError();
        store.save(name,"third",c);if(!Arrays.equals(store.load(true).state,a)||!new File(root,"resume.save.corrupt").isFile())throw new AssertionError("Corrupt primary overwrote backup");
        Files.write(new File(root,"resume.save.tmp").toPath(),new byte[]{1,2,3});
        if(!Arrays.equals(store.load(false).state,c))throw new AssertionError("Interrupted write affected committed checkpoint");
        byte[] cached=store.scene(name,new File(run,"cache"));if(!Arrays.equals(scene,cached))throw new AssertionError();
        File sceneFile=new File(new File(root,"scenes"),name);byte[] changed=scene.clone();changed[8]^=1;Files.write(sceneFile.toPath(),changed);
        reject(() -> store.scene(name,new File(run,"cache")));store.cache(scene);if(!Arrays.equals(scene,store.scene(name,new File(run,"cache"))))throw new AssertionError("Scene cache could not be repaired");
        reject(() -> store.scene("../resume.save",new File(run,"cache")));
        Files.write(primary.toPath(),Arrays.copyOf(corrupt,10));reject(() -> store.load(false));
        store.quarantine(false);if(!Arrays.equals(store.load(true).state,a))throw new AssertionError();
        if(!store.lastScene().equals(name))throw new AssertionError();
        store.saveNamed(CheckpointStore.SAFE,name,"quiet",a);
        store.saveNamed(CheckpointStore.MANUAL,name,"manual combat",b);
        store.saveNamed(CheckpointStore.ENTRY,name,"map entry",c);
        store.save(name,"unsafe pause",b);store.save(name,"unsafe background",c);
        CheckpointStore reopened=new CheckpointStore(run);
        if(!Arrays.equals(reopened.load(CheckpointStore.SAFE).state,a)||!Arrays.equals(reopened.load(CheckpointStore.MANUAL).state,b))throw new AssertionError("Resume displaced stable/manual slots");
        store.saveNamed(CheckpointStore.SAFE,name,"next quiet",b);
        if(!Arrays.equals(store.load(CheckpointStore.SAFE_PREVIOUS).state,a))throw new AssertionError();
        byte[] goodSafe=Files.readAllBytes(new File(root,CheckpointStore.SAFE).toPath());
        File blockedSafe=new File(root,"safe.save.tmp");blockedSafe.mkdir();File childSafe=new File(blockedSafe,"keep");Files.write(childSafe.toPath(),a);
        reject(()->store.saveNamed(CheckpointStore.SAFE,name,"disk full",c));
        if(!Arrays.equals(goodSafe,Files.readAllBytes(new File(root,CheckpointStore.SAFE).toPath()))||!Arrays.equals(store.load(CheckpointStore.SAFE_PREVIOUS).state,a))throw new AssertionError("Failed safe write rotated history");
        Files.delete(childSafe.toPath());Files.delete(blockedSafe.toPath());
        goodSafe[goodSafe.length-1]^=1;Files.write(new File(root,CheckpointStore.SAFE).toPath(),goodSafe);
        store.saveNamed(CheckpointStore.SAFE,name,"repair",c);
        if(!Arrays.equals(store.load(CheckpointStore.SAFE_PREVIOUS).state,a))throw new AssertionError("Corrupt safe replaced fallback");
        CheckpointStore.Record origin=store.load(CheckpointStore.MANUAL);
        if(store.quickDeaths(origin,true)!=1||new CheckpointStore(run).quickDeaths(origin,true)!=2||store.quickDeaths(origin,false)!=2)throw new AssertionError("Death loop not persistent");
        if(store.quickDeaths(store.load(CheckpointStore.ENTRY),false)!=0)throw new AssertionError("Different save inherited death loop");
        reject(()->store.load("../safe.save"));
        File manualFile=new File(root,CheckpointStore.MANUAL);Files.write(manualFile.toPath(),new byte[]{1});
        store.saveNamed(CheckpointStore.MANUAL,name,"manual repair",b);
        if(!Arrays.equals(store.load(CheckpointStore.MANUAL).state,b))throw new AssertionError("Explicit save could not replace corrupt manual slot");
        System.out.println("{\"passed\":true,\"two_generations\":true,\"corrupt_primary_preserves_previous\":true,\"interrupted_temp_ignored\":true,\"scene_integrity_and_repair\":true,\"truncation_and_path_escape_rejected\":true,\"primary_and_backup_write_failures_preserve_both_generations\":true,\"staged_payload_cleanup\":true,\"retry_after_write_failure\":true,\"duplicate_preserves_previous\":true}");
    }
}
