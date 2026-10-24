package org.rcport.diagnostic;
import java.io.*;
import java.nio.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Optional checksummed audio sidecars, keyed by the exact existing gameplay record. */
final class AudioCheckpointStore {
    static final int LIMIT=256*1024,KEEP=12;
    private final File root;
    AudioCheckpointStore(File files){root=new File(files,"audio-checkpoints");}
    static long cursor(byte[] packet)throws IOException{
        if(packet.length!=16 || !new String(packet,0,8,java.nio.charset.StandardCharsets.US_ASCII).equals("RCCURS01"))throw new IOException("Audio cursor packet");
        long value=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN).getLong(8);if(value<0)throw new IOException("Audio cursor range");return value;
    }
    private static byte[] hash(byte[] bytes)throws IOException{try{return MessageDigest.getInstance("SHA-256").digest(bytes);}catch(NoSuchAlgorithmException e){throw new IOException(e);}}
    static byte[] bankHash(byte[] bytes)throws IOException{return hash(bytes);}
    private static String identity(String scene,byte[] state)throws IOException{if(state.length<20 || state.length>CheckpointStore.LIMIT)throw new IOException("Gameplay state size");return CheckpointStore.identity(new CheckpointStore.Record(scene,"",0,state));}
    private File target(String identity)throws IOException{if(!identity.matches("[0-9a-f]{64}"))throw new IOException("Audio identity");return new File(root,identity+".audio");}
    private static void snapshot(byte[] bytes)throws IOException{
        if(bytes==null || bytes.length<12 || bytes.length>LIMIT || !new String(bytes,0,8,java.nio.charset.StandardCharsets.US_ASCII).equals("RCRES001") || ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).getInt(8)!=bytes.length-12)throw new IOException("Audio snapshot envelope");
    }
    synchronized boolean save(String scene,byte[] state,byte[] bank,long expected,SceneAudioCapture.Result capture)throws IOException{
        if(capture==null || capture.cursor!=expected || expected<0)return false;
        if(bank.length!=32)throw new IOException("Bank digest size");byte[] bytes=capture.bytes();snapshot(bytes);String id=identity(scene,state);
        ByteArrayOutputStream body=new ByteArrayOutputStream();try(DataOutputStream out=new DataOutputStream(body)){out.writeUTF(id);out.write(bank);out.writeLong(expected);out.writeInt(bytes.length);out.write(bytes);}
        byte[] payload=body.toByteArray();ByteArrayOutputStream encoded=new ByteArrayOutputStream();try(DataOutputStream out=new DataOutputStream(encoded)){out.writeLong(0x5243415544533031L);out.writeInt(payload.length);out.write(hash(payload));out.write(payload);}
        CheckpointStore.atomic(target(id),encoded.toByteArray());prune();return true;
    }
    synchronized SceneAudioCapture.Result load(String scene,byte[] state,byte[] bank,long expected)throws IOException{
        String id=identity(scene,state);File file=target(id);if(!file.isFile())return null;
        if(bank.length!=32)throw new IOException("Bank digest size");long size=file.length();if(size<44 || size>LIMIT+256)throw new IOException("Audio sidecar size");byte[] encoded=new byte[(int)size];try(DataInputStream bounded=new DataInputStream(new FileInputStream(file))){bounded.readFully(encoded);if(bounded.read()!=-1)throw new IOException("Audio sidecar changed");}
        try(DataInputStream in=new DataInputStream(new ByteArrayInputStream(encoded))){
            if(in.readLong()!=0x5243415544533031L || in.readInt()!=encoded.length-44)throw new IOException("Audio sidecar format");byte[] digest=new byte[32];in.readFully(digest);byte[] payload=new byte[encoded.length-44];in.readFully(payload);if(!MessageDigest.isEqual(digest,hash(payload)))throw new IOException("Audio sidecar checksum");
            try(DataInputStream data=new DataInputStream(new ByteArrayInputStream(payload))){
                String actual=data.readUTF();byte[] actualBank=new byte[32];data.readFully(actualBank);long sequence=data.readLong();int n=data.readInt();
                if(!actual.equals(id) || !MessageDigest.isEqual(actualBank,bank) || sequence!=expected || sequence<0)return null;
                if(n<12 || n>LIMIT)throw new IOException("Audio snapshot size");byte[] bytes=new byte[n];data.readFully(bytes);if(data.read()!=-1)throw new IOException("Audio sidecar trailing bytes");snapshot(bytes);return new SceneAudioCapture.Result(sequence,bytes);
            }
        }
    }
    private void prune()throws IOException{
        synchronized(CheckpointStore.class){
        File[] stale=root.listFiles(f->f.isFile() && f.getName().matches("[0-9a-f]{64}\\.audio\\.tmp"));if(stale!=null)for(File file:stale)Files.deleteIfExists(file.toPath());
        File[] files=root.listFiles(f->f.isFile() && f.getName().matches("[0-9a-f]{64}\\.audio"));if(files==null)return;
        Arrays.sort(files,Comparator.comparingLong(File::lastModified).reversed().thenComparing(File::getName));for(int i=KEEP;i<files.length;i++)Files.deleteIfExists(files[i].toPath());
        }
    }
}
