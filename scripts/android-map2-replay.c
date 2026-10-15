/* Actual map1 input completion -> production map2 travel and persisted restore. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_squad_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_observe(uint64_t,double*,size_t);
extern int32_t rc_game_travel(uint64_t,const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_restore_handoff(uint64_t,const uint8_t*,size_t,char*,size_t);
extern intptr_t rc_game_map_handoff(uint64_t,uint8_t*,size_t);
extern intptr_t rc_game_audio(uint64_t,uint32_t,uint8_t*,size_t);
extern intptr_t rc_game_draw(uint64_t,size_t,size_t,uint32_t*,size_t);
extern int32_t rc_game_command(uint64_t,uint32_t);
extern void rc_game_release(uint64_t);
static uint8_t*read_file(const char*path,size_t*n){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0);rewind(f);uint8_t*b=malloc((size_t)size);assert(b);assert(fread(b,1,(size_t)size,f)==(size_t)size);fclose(f);*n=(size_t)size;return b;}
int main(int argc,char**argv){
 assert(argc==4);size_t n1,n2,ni;uint8_t*one=read_file(argv[1],&n1),*two=read_file(argv[2],&n2),*input=read_file(argv[3],&ni);char error[512]={0};
 uint64_t h=rc_game_create(one,n1,error,sizeof(error));assert(h);uint32_t initial[10],status[10],completed[10],entry[10],squad[12],prior_squad[12];
 assert(rc_game_status(h,initial,10)==10);assert(rc_game_travel(h,two,n2,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,initial,sizeof(status)));
 assert(ni>=12 && !memcmp(input,"RCINPUT1",8));uint32_t count;memcpy(&count,input+8,4);assert(ni==12+(size_t)count*20);
 for(uint32_t i=0;i<count;i++){float v[4];uint32_t flags;memcpy(v,input+12+i*20,16);memcpy(&flags,input+28+i*20,4);assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);}
 assert(rc_game_status(h,completed,10)==10 && completed[4]==1 && completed[0]==20);assert(rc_game_squad_status(h,prior_squad,12)==12);
 intptr_t hn=rc_game_map_handoff(h,NULL,0);assert(hn>0);uint8_t*handoff=malloc((size_t)hn);assert(handoff && rc_game_map_handoff(h,handoff,(size_t)hn)==hn);
 FILE*f=fopen("map1-handoff.json","wb");assert(f && fwrite(handoff,1,(size_t)hn,f)==(size_t)hn);fclose(f);
 assert(rc_game_travel(h,one,n1,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,completed,sizeof(status)));
 assert(rc_game_travel(h,(const uint8_t*)"invalid",7,error,sizeof(error))==-1);assert(rc_game_status(h,status,10)==10 && !memcmp(status,completed,sizeof(status)));
 assert(rc_game_travel(h,two,n2,error,sizeof(error))==0);assert(rc_game_status(h,entry,10)==10 && entry[4]==0 && entry[5]==0 && !memcmp(entry,completed,3*sizeof(uint32_t)));
 assert(rc_game_squad_status(h,squad,12)==12 && !memcmp(squad,prior_squad,4*sizeof(uint32_t)));
 double pose[6];assert(rc_game_observe(h,pose,6)==6 && fabs(pose[0]+10640.3818359375)<0.0001 && fabs(pose[2]+808.0003662109375)<0.0001);
 uint64_t restored=rc_game_create(two,n2,error,sizeof(error));assert(restored && rc_game_restore_handoff(restored,handoff,(size_t)hn,error,sizeof(error))==0);assert(rc_game_status(restored,status,10)==10 && !memcmp(status,entry,sizeof(status)));rc_game_release(restored);
 uint32_t sounds=0;
 for(int i=0;i<360;i++){
  uint32_t flags=(i>=180&&i<192?2u:0u)|(i==210?4u:0u)|(i==90?1u:0u)|(i>=240&&i<270?64u:0u);
  assert(rc_game_advance(h,1.0/60,i<120?.5f:0.f,i>=120&&i<180?.5f:0.f,0.f,0.f,flags)==1);
  intptr_t n=rc_game_audio(h,1,NULL,0);assert(n>=12);uint8_t*b=malloc((size_t)n);assert(b && rc_game_audio(h,1,b,(size_t)n)==n);uint32_t events;memcpy(&events,b+8,4);sounds+=events;free(b);
 }
 assert(rc_game_status(h,status,10)==10 && status[4]==0 && status[6]>0 && status[1]==20 && status[2]<entry[2] && status[8]>0 && sounds>0);
 uint32_t*pixels=malloc(320*200*4);assert(pixels && rc_game_draw(h,320,200,pixels,320*200)==320*200);f=fopen("map2.argb","wb");assert(f && fwrite(pixels,4,320*200,f)==320*200);fclose(f);free(pixels);
 assert(rc_game_command(h,1)==0 && rc_game_status(h,status,10)==10 && !memcmp(status,entry,sizeof(status)));
 printf("{\"passed\":true,\"real_map1_inputs\":%u,\"atomic_early_wrong_corrupt_travel\":true,\"saved_restore\":true,\"player_health\":%u,\"player_ammo\":%u,\"scorch_health\":%u,\"original_weapon_events\":%u,\"map2_controls_and_reset\":true}\n",count,entry[0],entry[1],squad[0],sounds);
 rc_game_release(h);free(handoff);free(one);free(two);free(input);return 0;
}
