package org.rcport.diagnostic;
import java.io.IOException;
import java.nio.*;
/** Tracked slots at the last native reconciliation; reading never reaps or renders. */
final class ScenePoolStatus {
    final int capacity,active,oneshots,loops,pending;final boolean fault;
    final long frames,last,admitted,protectedRejected,capacityRejected,preempted,completed;
    private ScenePoolStatus(ByteBuffer b)throws IOException{
        capacity=b.getInt();active=b.getInt();oneshots=b.getInt();loops=b.getInt();pending=b.getInt();int failed=b.getInt();
        if(capacity!=16 || active<0 || active>capacity || oneshots<0 || loops<0 || oneshots>16 || loops>16 || oneshots+loops!=active || pending<0 || pending>512 || failed<0 || failed>1)throw new IOException("Ungültige Poolbelegung");fault=failed!=0;
        long[] words=new long[7];for(int i=0;i<words.length;i++){words[i]=b.getLong();if(words[i]<0)throw new IOException("Ungültiger Poolzähler");}
        frames=words[0];last=words[1];admitted=words[2];protectedRejected=words[3];capacityRejected=words[4];preempted=words[5];completed=words[6];
    }
    static ScenePoolStatus decode(byte[] bytes)throws IOException{
        if(bytes==null || bytes.length!=88 || !new String(bytes,0,8,java.nio.charset.StandardCharsets.US_ASCII).equals("RCPOOL01"))throw new IOException("Ungültiges Poolstatuspaket");
        ByteBuffer b=ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);b.position(8);return new ScenePoolStatus(b);
    }
    @Override public String toString(){return "active="+active+" loops="+loops+" pending="+pending+" frames="+frames+" admitted="+admitted+" protected="+protectedRejected+" capacity="+capacityRejected+" preempted="+preempted+" completed="+completed+" fault="+fault;}
}
