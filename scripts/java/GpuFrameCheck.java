package org.rcport.diagnostic;
import java.nio.*;
/** Exercises the actual Android frame reader on the JVM, without creating a GL view. */
public final class GpuFrameCheck {
    static byte[] packet(String version,int[][] batches) {
        ByteBuffer b=ByteBuffer.allocate(4096).order(ByteOrder.LITTLE_ENDIAN);
        for(int i=0;i<10;i++) b.putInt(0);
        b.put(version.getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        b.putInt(640).putInt(400);
        for(int i=0;i<12;i++) b.putFloat(i%4==0 ? 1:0);
        b.putInt(batches.length).putInt(0);
        for(int[] batch:batches) {
            b.putInt(batch[0]).putInt(batch[1]);
            if(version.equals("RCFRA003")) b.putInt(batch[2]);
            b.putInt(3);
            for(int i=0;i<27;i++) b.putFloat(0.25f);
        }
        return java.util.Arrays.copyOf(b.array(),b.position());
    }
    static void check(boolean condition) {if(!condition) throw new AssertionError();}
    static void invalid(byte[] bytes) {
        try {new GpuSceneView.Frame(bytes,false);}
        catch(IllegalArgumentException | BufferUnderflowException expected) {return;}
        throw new AssertionError("Accepted invalid frame");
    }
    public static void main(String[] args) throws Exception {
        byte[] bytes=packet("RCFRA003",new int[][]{{0,-1,0},{1,0,1},{1,1,2},{1,0,1},{2,1,0}});
        GpuSceneView.Frame f=new GpuSceneView.Frame(bytes,false);
        check(f.actors.length==5 && f.actors[1].blend==1 && f.actors[2].blend==2);
        check(f.actors[1].texture==f.actors[3].texture && f.actors[4].pass==2);
        check(f.actors[1].vertices.length==108 && !f.capture);
        GpuSceneView.Frame rgb=new GpuSceneView.Frame(packet("RCFRA003",new int[][]{{1,0,3},{1,0,4},{1,0,5},{1,0,0x10000403},{1,0,0x00060301}}),false);
        check(rgb.actors.length==5 && rgb.actors[3].blend==0x10000403 && rgb.actors[4].blend==0x00060301);
        GpuSceneView.Frame old=new GpuSceneView.Frame(packet("RCFRA002",new int[][]{{0,-1,0},{1,0,0}}),true);
        check(old.actors[1].pass==2 && old.actors[1].blend==0 && old.capture);
        for(int[][] bad:new int[][][] {
            {{1,0,0}},{{0,0,1}},{{1,-1,1}},{{1,0,6}},{{2,0,1}},{{2,0,0},{1,0,1}},{{1,1024,1}},{{1,0,0x10000401}},{{1,0,0x00010003}},{{1,0,0x00000803}}
        }) invalid(packet("RCFRA003",bad));
        invalid(java.util.Arrays.copyOf(bytes,bytes.length-1));
        invalid(java.util.Arrays.copyOf(bytes,bytes.length+1));
        int worldPackets=0;
        for(String path:args) {
            byte[] nativeFrame=java.nio.file.Files.readAllBytes(java.nio.file.Path.of(path));
            if(path.endsWith(".world")) {
                GpuSceneView.World actualWorld=new GpuSceneView.World(nativeFrame);
                check(actualWorld.textures.length>0 && actualWorld.batches.length>0);
                worldPackets++;
                continue;
            }
            byte[] statusFrame=new byte[nativeFrame.length+40];
            System.arraycopy(nativeFrame,0,statusFrame,40,nativeFrame.length);
            GpuSceneView.Frame actual=new GpuSceneView.Frame(statusFrame,false);
            check(actual.camera.length==12 && actual.actors.length>0);
        }
        System.out.println("Native player camera frames parsed: "+(args.length-worldPackets)+"; native worlds: "+worldPackets);
        System.out.println("Actual GPU frame reader: legacy and effect packets passed");
    }
}
