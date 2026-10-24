package org.rcport.diagnostic;
import java.io.IOException;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
/** Bounded optional music envelopes. Older WAV-only scenes retain their byte contract. */
final class MissionSoundtrack {
    private static boolean magic(byte[] bytes,String value){return bytes.length>=8 && new String(bytes,0,8,StandardCharsets.US_ASCII).equals(value);}
    static byte[] waveBank(byte[] bytes)throws IOException{
        if(!magic(bytes,"RCSND002"))return bytes;
        if(bytes.length<24 || bytes.length>32*1024*1024+1036)throw new IOException("Soundtrack bank budget");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);int n=b.getInt(8);
        if(n<12 || n>8*1024*1024+1024 || n>bytes.length-24)throw new IOException("Soundtrack WAV bank length");
        byte[] wave=Arrays.copyOfRange(bytes,12,12+n);if(!magic(wave,"RCSND001"))throw new IOException("Nested WAV bank");
        int at=12+n;if(!new String(bytes,at,8,StandardCharsets.US_ASCII).equals("RCMUB001"))throw new IOException("Soundtrack bank magic");
        int count=b.getInt(at+8);at+=12;if(count<0 || count>24 || bytes.length-12-n>24*1024*1024)throw new IOException("Soundtrack count/budget");
        for(int i=0;i<count;i++){if(bytes.length-at<4)throw new IOException("Truncated soundtrack");int size=b.getInt(at);at+=4;if(size<4 || size>4*1024*1024 || size>bytes.length-at || !new String(bytes,at,4,StandardCharsets.US_ASCII).equals("OggS"))throw new IOException("Soundtrack Ogg length/magic");at+=size;}
        if(at!=bytes.length)throw new IOException("Trailing soundtrack data");return wave;
    }
    static byte[] pose(byte[] bytes)throws IOException{
        if(!magic(bytes,"RCLOOP05"))return bytes;
        if(bytes.length<124 || bytes.length>828)throw new IOException("Soundtrack pose size");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);int n=b.getInt(8);
        if(n<32 || n>736 || bytes.length!=12+n+80)throw new IOException("Soundtrack pose length");
        byte[] pose=Arrays.copyOfRange(bytes,12,12+n);if(!magic(pose,"RCLOOP04") || !new String(bytes,12+n,8,StandardCharsets.US_ASCII).equals("RCMUS001"))throw new IOException("Soundtrack pose magic");
        b.position(12+n+8);
        for(int i=0;i<2;i++){int track=b.getInt();long activation=b.getLong(),elapsed=b.getLong();float in=b.getFloat(),out=b.getFloat();int ik=b.getInt(),ok=b.getInt();if(track< -1 || track>=24 || activation<0 || elapsed<0 || elapsed>48000L*60*60*24 || !Float.isFinite(in) || !Float.isFinite(out) || in<0 || in>60 || out<0 || out>60 || (ik & ~0x103)!=0 || ok<0 || ok>3)throw new IOException("Soundtrack command");}
        return pose;
    }
}
