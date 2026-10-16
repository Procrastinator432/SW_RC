package org.rcport.diagnostic;

import java.util.HashMap;
import java.util.Map;

/** Native admission handles protected sources. The host stops replaced voice streams only. */
final class SoundOwnership {
    private final Map<Integer,Integer> voices=new HashMap<>();
    int replaced(int owner,int category) {
        if(category!=1) return 0;
        Integer previous=voices.remove(owner);return previous==null?0:previous;
    }
    void played(int owner,int category,int stream) {if(category==1 && stream!=0) voices.put(owner,stream);}
    int[] activeStreams() {
        int[] result=new int[voices.size()];int at=0;
        for(int stream:voices.values()) result[at++]=stream;
        return result;
    }
    void clear() {voices.clear();}
}
