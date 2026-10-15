package org.rcport.diagnostic;

import java.io.*;
import java.nio.file.*;
import java.security.*;
import java.util.*;
import java.util.zip.*;

/** Streaming package import; the atomic index is published only after all maps validate. */
final class CampaignStore {
    static final String[] MAPS={"GEO_01A","GEO_01B","GEO_01C","GEO_03A"};
    private final File files,index,scenes,staging;
    interface Validator {void validate(File scene,String map)throws IOException;}
    interface Progress {void update(String message);}
    CampaignStore(File files) {
        this.files=files;index=new File(files,"campaign/package.properties");
        scenes=new File(files,"checkpoints/scenes");staging=new File(files,"campaign/staging");
    }
    static String hash(File file)throws IOException {
        try {
            MessageDigest digest=MessageDigest.getInstance("SHA-256");byte[] block=new byte[65536];
            try(InputStream in=new FileInputStream(file)){int n;while((n=in.read(block))!=-1)digest.update(block,0,n);}
            StringBuilder out=new StringBuilder();for(byte b:digest.digest())out.append(String.format(Locale.ROOT,"%02x",b&255));return out.toString();
        }catch(NoSuchAlgorithmException e){throw new IOException(e);}
    }
    private static Properties properties(byte[] bytes)throws IOException {
        Properties result=new Properties(){@Override public synchronized Object put(Object key,Object value){if(containsKey(key))throw new IllegalArgumentException("Doppelter Paketeintrag");return super.put(key,value);}};
        try {result.load(new ByteArrayInputStream(bytes));}catch(IllegalArgumentException e){throw new IOException("Ungültiges Paketverzeichnis",e);}
        if(!"RCCAMPAIGN1".equals(result.getProperty("format")) || result.size()!=13)throw new IOException("Unvollständiges Kampagnenpaket (vier Karten erforderlich)");
        for(String map:MAPS){
            String filename=result.getProperty(map+".file","");String sha=result.getProperty(map+".sha256","");
            if(!filename.equals(map.toLowerCase(Locale.ROOT)+".rcplay") || !sha.matches("[0-9a-f]{64}"))throw new IOException("Ungültige Karte im Paket: "+map);
            length(result,map);
        }
        return result;
    }
    private static long length(Properties p,String map)throws IOException {
        try {long n=Long.parseLong(p.getProperty(map+".bytes",""));if(n<12 || n>SceneBytes.LIMIT)throw new NumberFormatException();return n;}
        catch(NumberFormatException e){throw new IOException("Ungültige Kartengröße: "+map);}
    }
    private Properties installed()throws IOException {
        if(index.length()<1 || index.length()>8192)throw new IOException("Szenenpaket fehlt oder ist beschädigt");
        return properties(Files.readAllBytes(index.toPath()));
    }
    boolean ready() {
        try {Properties p=installed();for(String map:MAPS)if(file(p,map).length()!=length(p,map))return false;return true;}
        catch(IOException e){return false;}
    }
    private File file(Properties p,String map)throws IOException {
        if(!Arrays.asList(MAPS).contains(map))throw new IOException("Unbekannte Kampagnenkarte");
        return new File(scenes,p.getProperty(map+".sha256")+".rcplay");
    }
    File scene(String map)throws IOException {
        Properties p=installed();File file=file(p,map);
        if(file.length()!=length(p,map) || !hash(file).equals(p.getProperty(map+".sha256")))throw new IOException("Die Karte "+map+" ist beschädigt. Szenenpaket erneut importieren.");
        return file;
    }
    void importPackage(InputStream source,Validator validator,Progress progress)throws IOException {
        synchronized(CampaignStore.class){importLocked(source,validator,progress);}
    }
    private void importLocked(InputStream source,Validator validator,Progress progress)throws IOException {
        if(!staging.isDirectory() && !staging.mkdirs())throw new IOException("Paketordner konnte nicht erstellt werden");
        File[] stale=staging.listFiles();if(stale!=null)for(File f:stale)if(f.isFile() && f.getName().startsWith("map-") && f.getName().endsWith(".tmp"))Files.delete(f.toPath());
        LinkedHashMap<String,File> prepared=new LinkedHashMap<>();
        try(ZipInputStream zip=new ZipInputStream(new BufferedInputStream(source))) {
            ZipEntry entry=zip.getNextEntry();
            if(entry==null || !entry.getName().equals("campaign.properties"))throw new IOException("Kein Szenenpaket: campaign.properties muss zuerst stehen");
            ByteArrayOutputStream small=new ByteArrayOutputStream();byte[] block=new byte[65536];int n;
            while((n=zip.read(block))!=-1){if(small.size()+n>8192)throw new IOException("Paketverzeichnis ist zu groß");small.write(block,0,n);}
            Properties manifest=properties(small.toByteArray());zip.closeEntry();
            while((entry=zip.getNextEntry())!=null){
                String map=null;for(String candidate:MAPS)if(entry.getName().equals(manifest.getProperty(candidate+".file")))map=candidate;
                if(map==null || entry.isDirectory() || prepared.containsKey(map))throw new IOException("Unzulässiger oder doppelter Paketeintrag: "+entry.getName());
                progress.update("Importiere "+map+" …");File temporary=File.createTempFile("map-",".tmp",staging);prepared.put(map,temporary);
                long total=0,expected=length(manifest,map);
                try(FileOutputStream out=new FileOutputStream(temporary)){
                    while((n=zip.read(block))!=-1){total+=n;if(total>expected)throw new IOException("Kartengröße überschritten: "+map);out.write(block,0,n);}out.getFD().sync();
                }
                zip.closeEntry();
                if(total!=expected || !hash(temporary).equals(manifest.getProperty(map+".sha256")))throw new IOException("Prüfsumme/Größe stimmt nicht: "+map);
                progress.update("Prüfe "+map+" …");validator.validate(temporary,map);
            }
            if(prepared.size()!=4)throw new IOException("Szenenpaket ist unvollständig");
            if(!scenes.isDirectory() && !scenes.mkdirs())throw new IOException("Szenenspeicher konnte nicht erstellt werden");
            for(Map.Entry<String,File> map:prepared.entrySet())Files.move(map.getValue().toPath(),file(manifest,map.getKey()).toPath(),StandardCopyOption.ATOMIC_MOVE,StandardCopyOption.REPLACE_EXISTING);
            CheckpointStore.atomic(index,small.toByteArray());
        } finally {for(File f:prepared.values())Files.deleteIfExists(f.toPath());}
    }
}
