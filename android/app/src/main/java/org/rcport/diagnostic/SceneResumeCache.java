package org.rcport.diagnostic;

/** In-memory pause continuity, scoped to this scene object. Reset wins against late captures. */
final class SceneResumeCache {
    private long version;
    private byte[] snapshot;
    synchronized long version(){return version;}
    synchronized byte[] read(long expected){return version==expected && snapshot!=null?snapshot.clone():null;}
    synchronized void keep(long expected,byte[] bytes){
        if(bytes!=null && bytes.length>256*1024)throw new IllegalArgumentException("Mission audio resume size");
        if(version==expected)snapshot=bytes==null?null:bytes.clone();
    }
    synchronized void clear(){version++;snapshot=null;}
}
