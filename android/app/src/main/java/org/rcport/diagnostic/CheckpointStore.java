package org.rcport.diagnostic;

import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;

/** Private immutable scene cache and two crash-safe, checksummed save generations. */
final class CheckpointStore {
    static final int LIMIT=8*1024*1024;
    private final File root,scenes;
    static final class Record {
        final String scene,title;
        final long savedAt;
        final byte[] state;
        Record(String scene,String title,long savedAt,byte[] state) {
            this.scene=scene;this.title=title;this.savedAt=savedAt;this.state=state;
        }
    }
    CheckpointStore(File files) {root=new File(files,"checkpoints");scenes=new File(root,"scenes");}
    boolean hasSave() {return slot(false).isFile() || slot(true).isFile();}
    boolean hasPrevious() {return slot(true).isFile();}
    private File slot(boolean previous) {return new File(root,previous?"previous.save":"resume.save");}
    private static byte[] digest(byte[] b) throws IOException {
        try{return MessageDigest.getInstance("SHA-256").digest(b);}catch(NoSuchAlgorithmException e){throw new IOException(e);}
    }
    private static String hex(byte[] b) {
        StringBuilder s=new StringBuilder();for(byte v:b)s.append(String.format(Locale.ROOT,"%02x",v&255));return s.toString();
    }
    private static File stage(File target,byte[] bytes) throws IOException {
        File parent=target.getParentFile();if(!parent.isDirectory() && !parent.mkdirs())throw new IOException("Speicherordner konnte nicht erstellt werden");
        File temp=new File(parent,target.getName()+".tmp");
        boolean complete=false;
        try {
            try(FileOutputStream out=new FileOutputStream(temp)){out.write(bytes);out.getFD().sync();}
            complete=true;return temp;
        } finally {if(!complete)Files.deleteIfExists(temp.toPath());}
    }
    private static void commit(File temp,File target) throws IOException {
        Files.move(temp.toPath(),target.toPath(),StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);
    }
    static synchronized void atomic(File target,byte[] bytes) throws IOException {
        File temp=stage(target,bytes);
        try {commit(temp,target);} finally {Files.deleteIfExists(temp.toPath());}
    }
    private static byte[] read(File file,int limit) throws IOException {
        long n=file.length();if(n<1 || n>limit)throw new IOException("Gespeicherte Datei ist leer oder zu groß");
        byte[] bytes=new byte[(int)n];try(DataInputStream in=new DataInputStream(new FileInputStream(file))){in.readFully(bytes);if(in.read()!=-1)throw new IOException("Gespeicherte Datei wurde verändert");}return bytes;
    }
    private File sceneFile(String name) throws IOException {
        if(name==null || !name.matches("[0-9a-f]{64}\\.rcplay"))throw new IOException("Ungültige Szenenreferenz im Spielstand");
        return new File(scenes,name);
    }
    String cache(byte[] bytes) throws IOException {
        String name=hex(digest(bytes))+".rcplay";File file=sceneFile(name);
        boolean valid=false;
        if(file.isFile() && file.length()==bytes.length) {
            try {MessageDigest hash=MessageDigest.getInstance("SHA-256");try(InputStream in=new FileInputStream(file)){byte[] block=new byte[65536];int n;while((n=in.read(block))!=-1)hash.update(block,0,n);}valid=name.equals(hex(hash.digest())+".rcplay");}catch(NoSuchAlgorithmException e){throw new IOException(e);}
        }
        if(!valid)atomic(file,bytes);
        return name;
    }
    byte[] scene(String name,File cache) throws IOException {
        File file=sceneFile(name);final byte[] b;
        try(InputStream in=new FileInputStream(file)){b=SceneBytes.read(in,file.length(),cache);}
        if(!name.equals(hex(digest(b))+".rcplay"))throw new IOException("Die gespeicherte Szenendatei ist beschädigt");return b;
    }
    void markScene(String name) throws IOException {sceneFile(name);atomic(new File(root,"last-scene"),name.getBytes(java.nio.charset.StandardCharsets.US_ASCII));}
    String lastScene() throws IOException {
        String name=new String(read(new File(root,"last-scene"),128),java.nio.charset.StandardCharsets.US_ASCII);sceneFile(name);return name;
    }
    Record load(boolean previous) throws IOException {return decode(read(slot(previous),LIMIT+8192));}
    private Record decode(byte[] b) throws IOException {
        if(b.length<44)throw new IOException("Checkpoint ist unvollständig");
        try(DataInputStream in=new DataInputStream(new ByteArrayInputStream(b))) {
            if(in.readLong()!=0x5243534c4f543031L)throw new IOException("Unbekanntes Checkpoint-Format");
            int n=in.readInt();byte[] hash=new byte[32];in.readFully(hash);
            if(n!=b.length-44)throw new IOException("Checkpoint ist unvollständig");byte[] body=new byte[n];in.readFully(body);
            if(!MessageDigest.isEqual(hash,digest(body)))throw new IOException("Checkpoint-Prüfsumme stimmt nicht");
            try(DataInputStream data=new DataInputStream(new ByteArrayInputStream(body))) {
                String scene=data.readUTF();sceneFile(scene);long savedAt=data.readLong();String title=data.readUTF();int size=data.readInt();
                if(size<20 || size>LIMIT || title.length()>4096 || savedAt<0)throw new IOException("Ungültiger Checkpoint-Inhalt");
                byte[] state=new byte[size];data.readFully(state);if(data.read()!=-1)throw new IOException("Zusatzdaten im Checkpoint");return new Record(scene,title,savedAt,state);
            }
        }
    }
    void save(String scene,String title,byte[] state) throws IOException {synchronized(CheckpointStore.class){saveLocked(scene,title,state);}}
    private void saveLocked(String scene,String title,byte[] state) throws IOException {
        sceneFile(scene);if(state.length<20 || state.length>LIMIT || title.length()>4096)throw new IOException("Checkpoint ist zu groß");
        // Never rotate a corrupt primary over a valid previous generation.
        byte[] old=null;
        if(slot(false).isFile()) {try {byte[] b=read(slot(false),LIMIT+8192);Record r=decode(b);if(r.scene.equals(scene)&&Arrays.equals(r.state,state))return;old=b;}catch(IOException ignored){quarantine(slot(false));}}
        ByteArrayOutputStream payload=new ByteArrayOutputStream();try(DataOutputStream data=new DataOutputStream(payload)){data.writeUTF(scene);data.writeLong(System.currentTimeMillis());data.writeUTF(title);data.writeInt(state.length);data.write(state);}
        byte[] body=payload.toByteArray();ByteArrayOutputStream encoded=new ByteArrayOutputStream();try(DataOutputStream data=new DataOutputStream(encoded)){data.writeLong(0x5243534c4f543031L);data.writeInt(body.length);data.write(digest(body));data.write(body);}
        // Finish the new payload before touching either committed generation. In
        // particular, disk-full/write failures must not discard the older backup.
        File temp=stage(slot(false),encoded.toByteArray());
        try {
            if(old!=null)atomic(slot(true),old);
            commit(temp,slot(false));
        } finally {Files.deleteIfExists(temp.toPath());}
        markScene(scene);
    }
    void quarantine(boolean previous) throws IOException {quarantine(slot(previous));}
    static void quarantine(File file) throws IOException {
        if(file.isFile())Files.move(file.toPath(),new File(file.getParentFile(),file.getName()+".corrupt").toPath(),StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);
    }
}
