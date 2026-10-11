package org.rcport.diagnostic;

import android.content.Context;
import android.graphics.Bitmap;
import android.opengl.GLES20;
import android.opengl.GLSurfaceView;
import android.util.Log;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.*;
import java.util.*;
import javax.microedition.khronos.egl.EGLConfig;
import javax.microedition.khronos.opengles.GL10;

/** GL thread owns every driver object. Immutable CPU descriptions survive context loss.
 * A single latest-frame mailbox bounds queued memory when drawing falls behind simulation. */
final class GpuSceneView extends GLSurfaceView implements GLSurfaceView.Renderer {
    static final int WIDTH=640, HEIGHT=400, STRIDE=36;
    static final class Texture {
        final int width,height; final byte[] rgba;
        Texture(int w,int h,byte[] bytes) {width=w;height=h;rgba=bytes;}
    }
    static final class Batch {
        final int texture,count,pass; final byte[] vertices;
        Batch(int t,int n,byte[] v) {this(t,n,v,0);}
        Batch(int t,int n,byte[] v,int p) {texture=t;count=n;vertices=v;pass=p;}
    }
    static final class World {
        final Texture[] textures; final Batch[] batches;
        World(byte[] packet) {
            ByteBuffer b=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN);
            magic(b,"RCGPU001"); int n=count(b,1024); textures=new Texture[n];
            for(int i=0;i<n;i++) {
                int w=count(b,4096),h=count(b,4096);
                if(w==0 || h==0 || (long)w*h*4>b.remaining()) throw new IllegalArgumentException("Invalid texture dimensions");
                textures[i]=new Texture(w,h,copy(b,w*h*4));
            }
            n=count(b,1025); batches=new Batch[n];
            for(int i=0;i<n;i++) {
                int t=b.getInt(),v=count(b,6000000);
                if(t < -1 || t>=textures.length || v%3!=0 || (long)v*STRIDE>b.remaining()) throw new IllegalArgumentException("Invalid geometry batch");
                batches[i]=new Batch(t,v,copy(b,v*STRIDE));
            }
            if(b.hasRemaining()) throw new IllegalArgumentException("Trailing world data");
        }
    }
    static final class Frame {
        final int[] status=new int[10]; final float[] camera=new float[12];
        final Batch[] actors; final float[] hud; final int hudCount; final boolean capture;
        Frame(byte[] packet,boolean snapshot) {
            capture=snapshot;
            ByteBuffer b=ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN);
            for(int i=0;i<10;i++) status[i]=b.getInt();
            byte[] signature=copy(b,8);
            String version=new String(signature,java.nio.charset.StandardCharsets.US_ASCII);
            boolean animated=version.equals("RCFRA002");
            if(!animated && !version.equals("RCFRA001")) throw new IllegalArgumentException("Unsupported frame version");
            if(b.getInt()!=WIDTH || b.getInt()!=HEIGHT) throw new IllegalArgumentException("Unsupported graphics dimensions");
            for(int i=0;i<12;i++) {camera[i]=b.getFloat();if(!Float.isFinite(camera[i])) throw new IllegalArgumentException("Invalid camera");}
            int n=count(b,animated ? 1024:4096); int rects=count(b,32768);
            if(animated) {
                actors=new Batch[n];int previousPass=0,total=0;
                for(int i=0;i<n;i++) {
                    int pass=count(b,1),texture=b.getInt(),vertices=count(b,300000);
                    total+=vertices;
                    if(pass<previousPass || texture < -1 || vertices%3!=0 || total>300000) throw new IllegalArgumentException("Invalid animated batch");
                    previousPass=pass;actors[i]=new Batch(texture,vertices,copy(b,vertices*STRIDE),pass);
                }
            } else {actors=new Batch[]{new Batch(-1,n*3,copy(b,n*3*STRIDE))};}
            hud=new float[rects*6*9];FloatBuffer rectangles=FloatBuffer.wrap(hud);
            for(int i=0;i<rects;i++) {
                int x=b.getInt(),y=b.getInt(),w=b.getInt(),h=b.getInt(),color=b.getInt();
                float l=2f*x/WIDTH-1,r=2f*(x+w)/WIDTH-1,t=1-2f*y/HEIGHT,d=1-2f*(y+h)/HEIGHT;
                vertex(rectangles,l,t,0,0,color);vertex(rectangles,l,d,0,0,color);vertex(rectangles,r,d,0,0,color);
                vertex(rectangles,l,t,0,0,color);vertex(rectangles,r,d,0,0,color);vertex(rectangles,r,t,0,0,color);
            }
            hudCount=rects*6;
            if(b.hasRemaining()) throw new IllegalArgumentException("Trailing frame data");
        }
    }
    private static void magic(ByteBuffer b,String expected) {
        for(int i=0;i<8;i++) if(b.get()!=(byte)expected.charAt(i)) throw new IllegalArgumentException("Unsupported graphics packet");
    }
    private static int count(ByteBuffer b,int max) {
        int n=b.getInt();if(n<0 || n>max) throw new IllegalArgumentException("Invalid graphics count");return n;
    }
    private static byte[] copy(ByteBuffer b,int count) {
        if(count<0 || count>b.remaining()) throw new IllegalArgumentException("Truncated graphics packet");
        byte[] out=new byte[count];b.get(out);return out;
    }
    private static void vertex(FloatBuffer out,float x,float y,float u,float v,int color) {
        out.put(x).put(y).put(0).put(u).put(v).put(((color>>16)&255)/255f)
            .put(((color>>8)&255)/255f).put((color&255)/255f).put((color>>>24)/255f);
    }
    private final boolean profiling,capture;
    private final java.util.function.Consumer<String> failure;
    private World world,uploaded;
    private Frame latest;
    private int program,position,uv,color,screen,textured,camera,fbo,depth,target;
    private int[] textures=new int[0],buffers=new int[0];
    private int stream,displayBuffer,surfaceWidth=1,surfaceHeight=1,contexts;
    private boolean failed;
    // One GL-thread staging allocation, reused across worlds, contexts and frames.
    // Large immutable arrays live on the managed heap so reloads exert real GC pressure.
    private ByteBuffer staging;
    private final FloatBuffer display=ByteBuffer.allocateDirect(6*STRIDE).order(ByteOrder.nativeOrder()).asFloatBuffer();
    GpuSceneView(Context context,boolean profile,boolean captureFrames,java.util.function.Consumer<String> onFailure) {
        super(context);profiling=profile;capture=captureFrames;failure=onFailure;
        setEGLContextClientVersion(2);setEGLConfigChooser(8,8,8,8,16,0);
        setPreserveEGLContextOnPause(false);setRenderer(this);setRenderMode(RENDERMODE_WHEN_DIRTY);
        setContentDescription("Spielbare Szene geo_01a GPU");
        vertex(display,-1,-1,0,0,0xffffffff);vertex(display,1,-1,1,0,0xffffffff);vertex(display,1,1,1,1,0xffffffff);
        vertex(display,-1,-1,0,0,0xffffffff);vertex(display,1,1,1,1,0xffffffff);vertex(display,-1,1,0,1,0xffffffff);display.flip();
    }
    synchronized void world(World next) {world=next;latest=null;requestRender();}
    synchronized void submit(Frame frame) {latest=frame;requestRender();}
    @Override public void onPause() {
        // Delete while this context is still current. This also avoids deferred driver
        // allocations accumulating over repeated DocumentsUI/context transitions.
        queueEvent(() -> {
            if(android.opengl.EGL14.eglGetCurrentContext().equals(android.opengl.EGL14.EGL_NO_CONTEXT)) return;
            GLES20.glDeleteTextures(textures.length,textures,0);GLES20.glDeleteBuffers(buffers.length,buffers,0);
            GLES20.glDeleteBuffers(2,new int[]{stream,displayBuffer},0);
            GLES20.glDeleteTextures(1,new int[]{target},0);GLES20.glDeleteRenderbuffers(1,new int[]{depth},0);
            GLES20.glDeleteFramebuffers(1,new int[]{fbo},0);GLES20.glDeleteProgram(program);
            textures=new int[0];buffers=new int[0];uploaded=null;
        });
        super.onPause();
    }
    private static int shader(int type,String source) {
        int shader=GLES20.glCreateShader(type);GLES20.glShaderSource(shader,source);GLES20.glCompileShader(shader);
        int[] ok=new int[1];GLES20.glGetShaderiv(shader,GLES20.GL_COMPILE_STATUS,ok,0);
        if(ok[0]==0) {String error=GLES20.glGetShaderInfoLog(shader);GLES20.glDeleteShader(shader);throw new IllegalStateException(error);}return shader;
    }
    @Override public void onSurfaceCreated(GL10 gl,EGLConfig config) {
        // The old context has already destroyed its objects. Never delete stale names in the new context.
        uploaded=null;textures=new int[0];buffers=new int[0];failed=false;contexts++;
        try {
            int vs=shader(GLES20.GL_VERTEX_SHADER,
                "attribute vec3 aPosition;attribute vec2 aUV;attribute vec4 aColor;uniform vec3 uCamera[4];uniform int uScreen;varying vec2 vUV;varying vec4 vColor;void main(){vUV=aUV;vColor=aColor;if(uScreen==1){gl_Position=vec4(aPosition,1.0);}else{vec3 d=aPosition-uCamera[0];float z=dot(d,uCamera[3]);gl_Position=vec4(dot(d,uCamera[1])*1.375,dot(d,uCamera[2])*2.2,z*1.0000008-8.0000032,z);}} ");
            int fs=shader(GLES20.GL_FRAGMENT_SHADER,
                "precision highp float;uniform sampler2D uTexture;uniform int uTextured;varying vec2 vUV;varying vec4 vColor;void main(){vec4 c=vColor;if(uTextured==1){vec4 t=texture2D(uTexture,fract(vUV));if(t.a<0.5)discard;c=vec4(t.rgb*vColor.rgb,1.0);}gl_FragColor=c;}");
            program=GLES20.glCreateProgram();GLES20.glAttachShader(program,vs);GLES20.glAttachShader(program,fs);GLES20.glLinkProgram(program);
            GLES20.glDeleteShader(vs);GLES20.glDeleteShader(fs);int[] ok=new int[1];GLES20.glGetProgramiv(program,GLES20.GL_LINK_STATUS,ok,0);
            if(ok[0]==0) throw new IllegalStateException(GLES20.glGetProgramInfoLog(program));
            position=GLES20.glGetAttribLocation(program,"aPosition");uv=GLES20.glGetAttribLocation(program,"aUV");color=GLES20.glGetAttribLocation(program,"aColor");
            screen=GLES20.glGetUniformLocation(program,"uScreen");textured=GLES20.glGetUniformLocation(program,"uTextured");camera=GLES20.glGetUniformLocation(program,"uCamera[0]");
            int[] names=new int[3];GLES20.glGenTextures(1,names,0);target=names[0];GLES20.glBindTexture(GLES20.GL_TEXTURE_2D,target);textureParameters();
            GLES20.glTexImage2D(GLES20.GL_TEXTURE_2D,0,GLES20.GL_RGBA,WIDTH,HEIGHT,0,GLES20.GL_RGBA,GLES20.GL_UNSIGNED_BYTE,null);
            GLES20.glGenRenderbuffers(1,names,0);depth=names[0];GLES20.glBindRenderbuffer(GLES20.GL_RENDERBUFFER,depth);
            String extensions=GLES20.glGetString(GLES20.GL_EXTENSIONS);
            GLES20.glRenderbufferStorage(GLES20.GL_RENDERBUFFER,extensions.contains("GL_OES_depth24") ? 0x81A6 : GLES20.GL_DEPTH_COMPONENT16,WIDTH,HEIGHT);
            GLES20.glGenFramebuffers(1,names,0);fbo=names[0];GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER,fbo);
            GLES20.glFramebufferTexture2D(GLES20.GL_FRAMEBUFFER,GLES20.GL_COLOR_ATTACHMENT0,GLES20.GL_TEXTURE_2D,target,0);
            GLES20.glFramebufferRenderbuffer(GLES20.GL_FRAMEBUFFER,GLES20.GL_DEPTH_ATTACHMENT,GLES20.GL_RENDERBUFFER,depth);
            if(GLES20.glCheckFramebufferStatus(GLES20.GL_FRAMEBUFFER)!=GLES20.GL_FRAMEBUFFER_COMPLETE) throw new IllegalStateException("Incomplete scene framebuffer");
            GLES20.glGenBuffers(2,names,0);stream=names[0];displayBuffer=names[1];GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER,displayBuffer);
            display.position(0);GLES20.glBufferData(GLES20.GL_ARRAY_BUFFER,display.remaining()*4,display,GLES20.GL_STATIC_DRAW);
            check();Log.i("RCGpu","context="+contexts+" renderer="+GLES20.glGetString(GLES20.GL_RENDERER));
        } catch(RuntimeException error) {fail(error);}
    }
    private static void textureParameters() {
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D,GLES20.GL_TEXTURE_MIN_FILTER,GLES20.GL_NEAREST);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D,GLES20.GL_TEXTURE_MAG_FILTER,GLES20.GL_NEAREST);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D,GLES20.GL_TEXTURE_WRAP_S,GLES20.GL_CLAMP_TO_EDGE);
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D,GLES20.GL_TEXTURE_WRAP_T,GLES20.GL_CLAMP_TO_EDGE);
    }
    @Override public void onSurfaceChanged(GL10 gl,int width,int height) {surfaceWidth=width;surfaceHeight=height;}
    private void upload(World next) {
        GLES20.glDeleteTextures(textures.length,textures,0);GLES20.glDeleteBuffers(buffers.length,buffers,0);
        textures=new int[next.textures.length];buffers=new int[next.batches.length];
        GLES20.glGenTextures(textures.length,textures,0);GLES20.glGenBuffers(buffers.length,buffers,0);
        int[] max=new int[1];GLES20.glGetIntegerv(GLES20.GL_MAX_TEXTURE_SIZE,max,0);
        for(int i=0;i<textures.length;i++) {
            Texture t=next.textures[i];if(t.width>max[0] || t.height>max[0]) throw new IllegalStateException("Texture exceeds device limit");
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D,textures[i]);textureParameters();
            GLES20.glTexImage2D(GLES20.GL_TEXTURE_2D,0,GLES20.GL_RGBA,t.width,t.height,0,GLES20.GL_RGBA,GLES20.GL_UNSIGNED_BYTE,stage(t.rgba));
        }
        int vertices=0;
        for(int i=0;i<buffers.length;i++) {
            Batch b=next.batches[i];GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER,buffers[i]);
            GLES20.glBufferData(GLES20.GL_ARRAY_BUFFER,b.count*STRIDE,stage(b.vertices),GLES20.GL_STATIC_DRAW);vertices+=b.count;
        }
        check();uploaded=next;Log.i("RCGpu","upload textures="+textures.length+" batches="+buffers.length+" vertices="+vertices);
    }
    private void attributes(int buffer) {
        GLES20.glBindBuffer(GLES20.GL_ARRAY_BUFFER,buffer);
        GLES20.glEnableVertexAttribArray(position);GLES20.glEnableVertexAttribArray(uv);GLES20.glEnableVertexAttribArray(color);
        GLES20.glVertexAttribPointer(position,3,GLES20.GL_FLOAT,false,STRIDE,0);
        GLES20.glVertexAttribPointer(uv,2,GLES20.GL_FLOAT,false,STRIDE,12);
        GLES20.glVertexAttribPointer(color,4,GLES20.GL_FLOAT,false,STRIDE,20);
    }
    private ByteBuffer staging(int bytes) {
        if(staging==null || staging.capacity()<bytes) staging=ByteBuffer.allocateDirect(Math.max(bytes,staging==null ? 1024 : staging.capacity()*2)).order(ByteOrder.nativeOrder());
        staging.clear();staging.limit(bytes);return staging;
    }
    private ByteBuffer stage(byte[] data) {ByteBuffer out=staging(data.length);out.put(data);out.flip();return out;}
    private ByteBuffer stage(float[] data) {ByteBuffer out=staging(data.length*4);out.asFloatBuffer().put(data);return out;}
    private void drawStream(ByteBuffer data,int count) {
        attributes(stream);GLES20.glBufferData(GLES20.GL_ARRAY_BUFFER,count*STRIDE,data,GLES20.GL_STREAM_DRAW);GLES20.glDrawArrays(GLES20.GL_TRIANGLES,0,count);
    }
    @Override public void onDrawFrame(GL10 gl) {
        if(failed) return;
        World next;Frame frame;synchronized(this) {next=world;frame=latest;}
        try {
            long begin=System.nanoTime();
            if(next==null && uploaded!=null) {
                GLES20.glDeleteTextures(textures.length,textures,0);GLES20.glDeleteBuffers(buffers.length,buffers,0);
                textures=new int[0];buffers=new int[0];uploaded=null;
            }
            if(next!=uploaded && next!=null) upload(next);
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER,fbo);GLES20.glViewport(0,0,WIDTH,HEIGHT);
            GLES20.glClearColor(16/255f,25/255f,35/255f,1);GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT|GLES20.GL_DEPTH_BUFFER_BIT);
            GLES20.glUseProgram(program);GLES20.glUniform1i(GLES20.glGetUniformLocation(program,"uTexture"),0);GLES20.glActiveTexture(GLES20.GL_TEXTURE0);
            if(frame!=null && next!=null) {
                GLES20.glEnable(GLES20.GL_DEPTH_TEST);GLES20.glDepthFunc(GLES20.GL_LESS);GLES20.glDisable(GLES20.GL_CULL_FACE);GLES20.glDisable(GLES20.GL_BLEND);
                GLES20.glUniform1i(screen,0);GLES20.glUniform3fv(camera,4,frame.camera,0);
                for(int i=0;i<buffers.length;i++) {
                    Batch b=next.batches[i];GLES20.glUniform1i(textured,b.texture<0 ? 0:1);
                    if(b.texture>=0) GLES20.glBindTexture(GLES20.GL_TEXTURE_2D,textures[b.texture]);
                    attributes(buffers[i]);GLES20.glDrawArrays(GLES20.GL_TRIANGLES,0,b.count);
                }
                int pass=0;
                for(Batch batch:frame.actors) {
                    if(batch.pass!=pass) {GLES20.glClear(GLES20.GL_DEPTH_BUFFER_BIT);pass=batch.pass;}
                    if(batch.texture>=textures.length) throw new IllegalArgumentException("Animated texture outside world atlas");
                    GLES20.glUniform1i(textured,batch.texture<0 ? 0:1);
                    if(batch.texture>=0) GLES20.glBindTexture(GLES20.GL_TEXTURE_2D,textures[batch.texture]);
                    drawStream(stage(batch.vertices),batch.count);
                }
                GLES20.glUniform1i(textured,0);
                GLES20.glDisable(GLES20.GL_DEPTH_TEST);GLES20.glUniform1i(screen,1);drawStream(stage(frame.hud),frame.hudCount);
                if(capture && frame.capture) saveReference(frame.status[5]);
            }
            GLES20.glBindFramebuffer(GLES20.GL_FRAMEBUFFER,0);GLES20.glViewport(0,0,surfaceWidth,surfaceHeight);
            GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT);GLES20.glDisable(GLES20.GL_DEPTH_TEST);GLES20.glUniform1i(screen,1);GLES20.glUniform1i(textured,1);
            int w=Math.min(surfaceWidth,surfaceHeight*WIDTH/HEIGHT),h=w*HEIGHT/WIDTH;
            GLES20.glViewport((surfaceWidth-w)/2,(surfaceHeight-h)/2,w,h);GLES20.glBindTexture(GLES20.GL_TEXTURE_2D,target);
            attributes(displayBuffer);GLES20.glDrawArrays(GLES20.GL_TRIANGLES,0,6);check();
            if(profiling && frame!=null) Log.i("RCGpu","{\"type\":\"draw\",\"submit_ns\":"+(System.nanoTime()-begin)+",\"ticks\":"+frame.status[5]+",\"context\":"+contexts+"}");
        } catch(RuntimeException error) {fail(error);}
    }
    private void saveReference(int ticks) {
        ByteBuffer pixels=staging(WIDTH*HEIGHT*4);GLES20.glReadPixels(0,0,WIDTH,HEIGHT,GLES20.GL_RGBA,GLES20.GL_UNSIGNED_BYTE,pixels);
        int[] argb=new int[WIDTH*HEIGHT];pixels.position(0);
        for(int y=HEIGHT-1;y>=0;y--) for(int x=0;x<WIDTH;x++) {
            int r=pixels.get()&255,g=pixels.get()&255,b=pixels.get()&255,a=pixels.get()&255;argb[y*WIDTH+x]=(a<<24)|(r<<16)|(g<<8)|b;
        }
        Bitmap bitmap=Bitmap.createBitmap(argb,WIDTH,HEIGHT,Bitmap.Config.ARGB_8888);
        File file=new File(getContext().getFilesDir(),"gpu-reference.png");
        try(FileOutputStream output=new FileOutputStream(file)) {bitmap.compress(Bitmap.CompressFormat.PNG,100,output);Log.i("RCGpu","reference ticks="+ticks+" file="+file);}
        catch(java.io.IOException e) {throw new IllegalStateException(e);} finally {bitmap.recycle();}
    }
    private static void check() {int error=GLES20.glGetError();if(error!=GLES20.GL_NO_ERROR) throw new IllegalStateException("OpenGL error "+error);}
    private void fail(RuntimeException error) {failed=true;Log.e("RCGpu","Graphics failure",error);post(() -> failure.accept(error.getMessage()));}
}
