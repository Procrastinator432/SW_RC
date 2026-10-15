#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_restore_handoff(uint64_t,const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_restore_checkpoint(uint64_t,const uint8_t*,size_t,char*,size_t);
extern intptr_t rc_game_checkpoint(uint64_t,uint8_t*,size_t);
extern intptr_t rc_game_graphics(uint64_t,uint32_t,uint8_t*,size_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_command(uint64_t,uint32_t);
extern void rc_game_release(uint64_t);
static uint8_t* read_file(const char*p,size_t*n){FILE*f=fopen(p,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0);rewind(f);uint8_t*b=malloc((size_t)size);assert(b&&fread(b,1,(size_t)size,f)==(size_t)size);fclose(f);*n=(size_t)size;return b;}
static void write_file(const char*p,const void*b,size_t n){FILE*f=fopen(p,"wb");assert(f&&fwrite(b,1,n,f)==n);fclose(f);}
static void state(uint64_t h,const char*prefix,int save){
 uint32_t words[10];assert(rc_game_status(h,words,10)==10);intptr_t n=rc_game_graphics(h,1,NULL,0);assert(n>0);uint8_t*b=malloc((size_t)n);assert(b&&rc_game_graphics(h,1,b,(size_t)n)==n);
 char path[80];snprintf(path,sizeof(path),"%s.status",prefix);
 if(save)write_file(path,words,sizeof(words));else{size_t k;uint8_t*old=read_file(path,&k);assert(k==sizeof(words)&&!memcmp(old,words,k));free(old);}
 snprintf(path,sizeof(path),"%s.graphics",prefix);
 if(save)write_file(path,b,(size_t)n);else{size_t k;uint8_t*old=read_file(path,&k);assert(k==(size_t)n&&!memcmp(old,b,k));free(old);}free(b);
}
static void play(uint64_t h,const uint8_t*b,uint32_t start,uint32_t end){for(uint32_t i=start;i<end;i++){float v[4];uint32_t flags;memcpy(v,b+12+i*20,16);memcpy(&flags,b+28+i*20,4);assert(rc_game_advance(h,1./60.,v[0],v[1],v[2],v[3],flags)==1);}}
int main(int argc,char**argv){
 assert(argc==5);size_t ns,nc,nh;uint8_t*scene=read_file(argv[2],&ns),*controls=read_file(argv[3],&nc),*handoff=read_file(argv[4],&nh);assert(nc==12+360*20);char error[512]={0};
 uint64_t h=rc_game_create(scene,ns,error,sizeof(error));assert(h);
 if(!strcmp(argv[1],"save")){
  assert(rc_game_restore_handoff(h,handoff,nh,error,sizeof(error))==0);state(h,"entry",1);play(h,controls,0,140);state(h,"checkpoint",1);
  intptr_t n=rc_game_checkpoint(h,NULL,0);assert(n>20);uint8_t*b=malloc((size_t)n);assert(b&&rc_game_checkpoint(h,b,(size_t)n)==n);write_file("checkpoint.bin",b,(size_t)n);free(b);
  play(h,controls,140,360);state(h,"continued",1);puts("{\"passed\":true,\"checkpoint_written\":true,\"baseline_continuation_written\":true}");
 } else {
  assert(!strcmp(argv[1],"resume"));size_t n;uint8_t*b=read_file("checkpoint.bin",&n);state(h,"fresh",1);
  b[n-1]^=1;assert(rc_game_restore_checkpoint(h,b,n,error,sizeof(error))==-1);state(h,"fresh",0);b[n-1]^=1;
  assert(rc_game_restore_checkpoint(h,b,n-1,error,sizeof(error))==-1);state(h,"fresh",0);
  assert(rc_game_restore_checkpoint(h,b,n,error,sizeof(error))==0);state(h,"checkpoint",0);play(h,controls,140,360);state(h,"continued",0);
  assert(rc_game_command(h,1)==0);state(h,"entry",0);free(b);
  puts("{\"passed\":true,\"separate_native_process\":true,\"exact_status_and_graphics\":true,\"continuation_exact\":true,\"entry_restart_exact\":true,\"corrupt_restore_atomic\":true}");
 }
 rc_game_release(h);free(scene);free(controls);free(handoff);return 0;
}
