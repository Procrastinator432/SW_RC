package org.rcport.diagnostic;

import java.io.*;

/** Bounded scene reader with a single payload allocation, including unknown-length providers. */
final class SceneBytes {
    static final int LIMIT=112*1024*1024;
    static byte[] read(InputStream input,long length,File cache) throws IOException {
        if(length>=0) return exact(input,length);
        File temporary=File.createTempFile("rc-scene-",".tmp",cache);
        try {
            long size=0;
            try(OutputStream output=new FileOutputStream(temporary)) {
                byte[] block=new byte[32768];int n;
                while((n=input.read(block))!=-1) {
                    size+=n;
                    if(size>LIMIT) throw new IOException("Szene überschreitet 112 MiB");
                    output.write(block,0,n);
                }
            }
            try(InputStream stored=new FileInputStream(temporary)) {return exact(stored,size);}
        } finally {if(!temporary.delete()) temporary.deleteOnExit();}
    }
    private static byte[] exact(InputStream input,long length) throws IOException {
        if(length<1 || length>LIMIT) throw new IOException("Ungültige Szenengröße (maximal 112 MiB)");
        byte[] bytes=new byte[(int)length];int at=0;
        while(at<bytes.length) {
            int n=input.read(bytes,at,bytes.length-at);
            if(n<0) throw new EOFException("Szenendatei ist unvollständig");
            if(n==0) {int value=input.read();if(value<0) throw new EOFException("Szenendatei ist unvollständig");bytes[at++]=(byte)value;}
            else at+=n;
        }
        if(input.read()!=-1) throw new IOException("Szenengröße wurde während des Lesens geändert");
        return bytes;
    }
}
