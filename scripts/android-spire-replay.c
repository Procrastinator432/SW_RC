/* Executes recorded normal C-ABI inputs on Android, with no state injection. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern intptr_t rc_game_draw(uint64_t,size_t,size_t,uint32_t*,size_t);
extern intptr_t rc_game_audio(uint64_t,uint32_t,uint8_t*,size_t);
extern intptr_t rc_game_mission_status(uint64_t,uint8_t*,size_t);
extern int32_t rc_game_command(uint64_t,uint32_t);
extern void rc_game_release(uint64_t);
static uint8_t *read_file(const char *path,size_t *size){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0);rewind(f);uint8_t*b=malloc((size_t)n);assert(b);assert(fread(b,1,(size_t)n,f)==(size_t)n);fclose(f);*size=(size_t)n;return b;}
int main(int argc,char**argv){
    assert(argc==3);size_t size,n;uint8_t*scene=read_file(argv[1],&size),*input=read_file(argv[2],&n);
    assert(n>=12 && !memcmp(input,"RCINPUT1",8));uint32_t count;memcpy(&count,input+8,4);assert(n==12+(size_t)count*20);
    char error[512]={0};uint64_t h=rc_game_create(scene,size,error,sizeof(error));if(!h){fprintf(stderr,"%s\n",error);return 1;}free(scene);
    uint32_t status[10],pixels[320*200],sounds[57]={0};
    for(uint32_t i=0;i<count;i++){
        float values[4];uint32_t flags;memcpy(values,input+12+i*20,16);memcpy(&flags,input+28+i*20,4);
        int steps=rc_game_advance(h,1.0/60,values[0],values[1],values[2],values[3],flags);assert(steps>=0);
        assert(rc_game_status(h,status,10)==10);assert(status[4]!=2);
        intptr_t bytes=rc_game_audio(h,1,NULL,0);assert(bytes>=12);uint8_t *audio=malloc((size_t)bytes);assert(audio);assert(rc_game_audio(h,1,audio,(size_t)bytes)==bytes);
        for(intptr_t at=12;at+24<=bytes;at+=24){uint32_t role;memcpy(&role,audio+at+16,4);assert(role<57);sounds[role]++;}free(audio);
        if(i%300==0 || i+1==count){assert(rc_game_draw(h,320,200,pixels,320*200)==320*200);}
    }
    assert(status[4]==1 && status[0]>0 && status[7]==5);
    for(int i=13;i<20;i++){if(i!=15 && i!=16)assert(sounds[i]==1);}assert(sounds[15]>0 && sounds[16]==0);
    uint8_t text[512]={0};intptr_t len=rc_game_mission_status(h,text,sizeof(text)-1);assert(len>0);assert(strstr((char*)text,"Turmeingang"));
    FILE*f=fopen("complete.argb","wb");assert(f);assert(fwrite(pixels,4,320*200,f)==320*200);fclose(f);
    printf("{\"passed\":true,\"android_native_input_replay\":true,\"inputs\":%u,\"health\":%u,\"kills\":%u,\"phase\":%u,\"cpu_frames\":true,\"mission_text\":\"%s\",\"sound_counts\":[",count,status[0],status[7],status[4],text);
    for(int i=20;i<33;i++)assert(sounds[i]==1);
    for(int i=33;i<57;i++){if(i!=38)assert(sounds[i]==1);}assert(sounds[38]==20);
    for(int i=0;i<57;i++)printf("%s%u",i?",":"",sounds[i]);printf("]}\n");
    assert(rc_game_command(h,1)==0);assert(rc_game_status(h,status,10)==10);assert(status[0]==25 && status[4]==0 && status[7]==0);
    rc_game_release(h);free(input);return 0;
}
