/* Input-only Android C ABI acceptance; no game-state injection. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_squad_status(uint64_t,uint32_t*,size_t);
extern intptr_t rc_game_audio(uint64_t,uint32_t,uint8_t*,size_t);
extern intptr_t rc_game_draw(uint64_t,size_t,size_t,uint32_t*,size_t);
extern int32_t rc_game_command(uint64_t,uint32_t);
extern void rc_game_release(uint64_t);
static uint8_t*read_file(const char*path,size_t*size){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>0);rewind(f);uint8_t*b=malloc((size_t)n);assert(b);assert(fread(b,1,(size_t)n,f)==(size_t)n);fclose(f);*size=(size_t)n;return b;}
int main(int argc,char**argv){assert(argc==4);size_t size;uint8_t*scene=read_file(argv[1],&size);printf("{\"passed\":true,\"normal_inputs_only\":true,\"runs\":[");
for(int run=0;run<2;run++){
 char error[512]={0};uint64_t h=rc_game_create(scene,size,error,512);if(!h){fprintf(stderr,"%s\n",error);return 1;}size_t bytes;uint8_t*input=read_file(argv[run+2],&bytes);assert(bytes>=12 && !memcmp(input,"RCINPUT1",8));uint32_t count;memcpy(&count,input+8,4);assert(bytes==12+(size_t)count*20);uint32_t status[10],squad[14],sounds[128]={0};
 for(uint32_t i=0;i<count;i++){float v[4];uint32_t flags;memcpy(v,input+12+i*20,16);memcpy(&flags,input+28+i*20,4);assert(rc_game_advance(h,1.0/60,v[0],v[1],v[2],v[3],flags)==1);assert(rc_game_status(h,status,10)==10 && status[4]!=2);
 if(i==0 || i==59 || i==1396 || i==1924){assert(status[0]==(i<1396?200u:25u));if(i<60){assert(status[1]==20 && status[6]==0);}}
  intptr_t n=rc_game_audio(h,1,NULL,0);assert(n>=12);uint8_t*a=malloc((size_t)n+16);assert(a);memset(a,0xa5,(size_t)n+16);assert(rc_game_audio(h,1,a,(size_t)n)==n);for(int k=0;k<16;k++)assert(a[n+k]==0xa5);for(intptr_t at=12;at+24<=n;at+=24){uint32_t role;memcpy(&role,a+at+16,4);assert(role<80);sounds[role]++;}free(a);
 }
 for(int i=65;i<76;i++)assert(sounds[i]==1);assert(sounds[76]==1 && sounds[77]==2 && sounds[78]==1 && sounds[79]==1);assert(sounds[0]==1 && sounds[1]==1);
 memset(squad,0xa5,sizeof(squad));assert(rc_game_squad_status(h,squad,11)==-1);for(int i=0;i<14;i++)assert(squad[i]==0xa5a5a5a5);assert(rc_game_squad_status(h,squad,12)==12);assert(squad[12]==0xa5a5a5a5 && squad[13]==0xa5a5a5a5);assert(squad[2]==1 && status[0]>0 && squad[4]==0);
 if(run==0){assert(status[4]==1 && status[7]==8);}else{assert(status[4]==0 && squad[7]>0 && squad[8]>0 && squad[9]>0 && sounds[64]>0);}
 uint32_t*pixels=calloc(320*200+4,4);assert(pixels);for(int i=320*200;i<320*200+4;i++)pixels[i]=0xa5a5a5a5;assert(rc_game_draw(h,320,200,pixels,320*200)==320*200);for(int i=320*200;i<320*200+4;i++)assert(pixels[i]==0xa5a5a5a5);FILE*f=fopen(run?"assist.argb":"complete.argb","wb");assert(f);assert(fwrite(pixels,4,320*200,f)==320*200);fclose(f);free(pixels);
 printf("%s{\"inputs\":%u,\"phase\":%u,\"health\":%u,\"kills\":%u,\"scorch_shots\":%u,\"scorch_hits\":%u,\"scorch_kills\":%u,\"scorch_sound_events\":%u}",run?",":"",count,status[4],status[0],status[7],squad[7],squad[8],squad[9],sounds[64]);
 assert(rc_game_command(h,1)==0);assert(rc_game_squad_status(h,squad,12)==12 && squad[2]==0 && squad[7]==0 && squad[9]==0);rc_game_release(h);free(input);
}free(scene);printf("],\"opening_injury_health_checked\":true,\"guarded_buffers\":true,\"restart\":true}\n");return 0;}
