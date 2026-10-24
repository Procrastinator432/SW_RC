package org.rcport.diagnostic;
import java.nio.*;
import java.util.Arrays;
/** User volume percentages; original per-source gains remain independent. */
final class MissionMix {
    private final int[] values;
    MissionMix(int... values){
        if(values.length!=4 && values.length!=5)throw new IllegalArgumentException("Fünf Lautstärkewerte erforderlich");
        this.values=Arrays.copyOf(values,5);if(values.length==4)this.values[4]=100;for(int value:values)if(value<0 || value>100)throw new IllegalArgumentException("Lautstärke außerhalb 0–100");
    }
    static MissionMix defaults(){return new MissionMix(100,100,100,100);}
    int value(int index){return values[index];}
    byte[] packet(){ByteBuffer b=ByteBuffer.allocate(28).order(ByteOrder.LITTLE_ENDIAN).put("RCMIX002".getBytes(java.nio.charset.StandardCharsets.US_ASCII));for(int value:values)b.putFloat(value/100f);return b.array();}
    static void validate(byte[] packet)throws java.io.IOException{
        if(!(packet.length==24 && new String(packet,0,8,java.nio.charset.StandardCharsets.US_ASCII).equals("RCMIX001") || packet.length==28 && new String(packet,0,8,java.nio.charset.StandardCharsets.US_ASCII).equals("RCMIX002")))throw new java.io.IOException("Ungültige Lautstärkeeinstellung");
        ByteBuffer b=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN);b.position(8);while(b.hasRemaining()){float value=b.getFloat();if(!Float.isFinite(value) || value<0 || value>1)throw new java.io.IOException("Ungültige Lautstärke");}
    }
    @Override public boolean equals(Object other){return other instanceof MissionMix && Arrays.equals(values,((MissionMix)other).values);}
    @Override public int hashCode(){return Arrays.hashCode(values);}
}
