package org.rcport.diagnostic;

import java.io.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Immutable, bounded Rust/Java wire packet. No Android dependencies. */
final class PcmPacket {
    static final int HEADER=40, MAX_FRAMES=4096, MAX_FILE=16*1024*1024;
    final int rate;
    final long epoch,sequence;
    private final short[] samples;
    PcmPacket(int rate,long epoch,long sequence,short[] samples) {
        if(rate<8000 || rate>192000 || epoch<=0 || sequence<0 || sequence==Long.MAX_VALUE
            || samples.length==0 || samples.length%2!=0 || samples.length>MAX_FRAMES*2)
            throw new IllegalArgumentException("Ungültiges PCM-Paket");
        this.rate=rate;this.epoch=epoch;this.sequence=sequence;this.samples=samples.clone();
    }
    int frames(){return samples.length/2;}
    short[] copySamples(){return samples.clone();}
    PcmPacket retag(long epoch,long sequence){return new PcmPacket(rate,epoch,sequence,samples);}
    static PcmPacket decode(byte[] bytes) throws IOException {
        if(bytes.length<HEADER || bytes.length>HEADER+MAX_FRAMES*4)throw new IOException("PCM-Länge");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
        byte[] magic=new byte[8];b.get(magic);
        if(!Arrays.equals(magic,"RCPCM001".getBytes(StandardCharsets.US_ASCII)))throw new IOException("PCM-Kennung");
        int rate=b.getInt(),channels=b.getShort(),bits=b.getShort();long epoch=b.getLong(),sequence=b.getLong();
        int frames=b.getInt(),size=b.getInt();
        if(channels!=2 || bits!=16 || frames<1 || frames>MAX_FRAMES || size!=frames*4 || b.remaining()!=size)
            throw new IOException("PCM-Format");
        short[] samples=new short[frames*2];for(int i=0;i<samples.length;i++)samples[i]=b.getShort();
        try{return new PcmPacket(rate,epoch,sequence,samples);}catch(IllegalArgumentException e){throw new IOException(e);}
    }
    static List<PcmPacket> readPackage(InputStream input) throws IOException {
        ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] chunk=new byte[8192];int count;
        while((count=input.read(chunk))!=-1){if(out.size()>MAX_FILE-count)throw new IOException("PCM-Datei zu groß");out.write(chunk,0,count);}
        byte[] bytes=out.toByteArray();if(bytes.length<16)throw new IOException("PCM-Dateikopf");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);byte[] magic=new byte[8];b.get(magic);
        if(!Arrays.equals(magic,"RCPCMSET".getBytes(StandardCharsets.US_ASCII)))throw new IOException("PCM-Dateikennung");
        int rate=b.getInt(),packets=b.getInt();
        if(packets<1 || packets>(bytes.length-16)/(HEADER+8))throw new IOException("PCM-Paketanzahl");
        List<PcmPacket> result=new ArrayList<>();
        for(int i=0;i<packets;i++){
            if(b.remaining()<4)throw new IOException("PCM-Paket fehlt");int length=b.getInt();
            if(length<HEADER || length>HEADER+MAX_FRAMES*4 || length>b.remaining())throw new IOException("PCM-Paketlänge");
            byte[] packet=new byte[length];b.get(packet);PcmPacket p=decode(packet);
            if(p.rate!=rate || p.epoch!=1 || p.sequence!=i)throw new IOException("PCM-Paketfolge");result.add(p);
        }
        if(b.hasRemaining())throw new IOException("PCM-Restbytes");return Collections.unmodifiableList(result);
    }
}
