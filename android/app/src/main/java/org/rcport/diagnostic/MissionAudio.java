package org.rcport.diagnostic;
import java.io.IOException;
interface MissionAudio extends AutoCloseable {
    boolean hasOriginalSounds();boolean ready()throws IOException;
    void events(byte[] bytes)throws IOException;void ambientLoops(byte[] bytes)throws IOException;
    void suspend();void resume();void stop();void reset();void close();
}
