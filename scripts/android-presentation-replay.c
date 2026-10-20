/* Contiguous Android-native acceptance; selected checkpoints feed the actual APK
 * GLES/SoundPool probe. No health, position, collision or mission-state fixtures. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern uint64_t rc_game_create(const uint8_t*,size_t,char*,size_t);
extern int32_t rc_game_advance(uint64_t,double,float,float,float,float,uint32_t);
extern int32_t rc_game_status(uint64_t,uint32_t*,size_t);
extern int32_t rc_game_energy_status(uint64_t);
extern intptr_t rc_game_checkpoint(uint64_t,uint8_t*,size_t);
extern int32_t rc_game_restore_checkpoint(uint64_t,const uint8_t*,size_t,char*,size_t);
extern intptr_t rc_game_graphics(uint64_t,uint32_t,uint8_t*,size_t);
extern int32_t rc_game_travel(uint64_t,const uint8_t*,size_t,char*,size_t);
extern void rc_game_release(uint64_t);
static uint8_t *read_file(const char *path,size_t *n){FILE*f=fopen(path,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long size=ftell(f);assert(size>0);rewind(f);uint8_t*b=malloc((size_t)size);assert(b && fread(b,1,(size_t)size,f)==(size_t)size);fclose(f);*n=(size_t)size;return b;}
static void write_file(const char *path,const void*b,size_t n){FILE*f=fopen(path,"wb");assert(f && fwrite(b,1,n,f)==n);fclose(f);}
static void advance(uint64_t h,const uint8_t*row){float v[4];uint32_t flags;memcpy(v,row,16);memcpy(&flags,row+16,4);int n=rc_game_advance(h,1./60.,v[0],v[1],v[2],v[3],flags);if(n!=1){uint32_t s[10];rc_game_status(h,s,10);fprintf(stderr,"advance=%d phase=%u tick=%u hp=%u\n",n,s[4],s[5],s[0]);abort();}}
int main(int argc,char**argv){
 assert(argc==5);size_t ns,nr;uint8_t*scene=read_file(argv[1],&ns),*rows=read_file(argv[2],&nr);char error[1024]={0};
 uint64_t h=rc_game_create(scene,ns,error,sizeof(error));if(!h){fprintf(stderr,"create: %s\n",error);return 1;}
 assert(nr>=12 && !memcmp(rows,"RCINPUT1",8));uint32_t count;memcpy(&count,rows+8,4);assert(nr==12+(size_t)count*20);
 const int energy_weapon=rc_game_energy_status(h)>=0;int minimum_energy=1000;
 uint32_t ticks[8]={0};const char*names[]={"intro","injury","bacta","barriers","falling","bunker","demolition","door"};uint32_t captures=0;
 {FILE*f=fopen(argv[4],"r");assert(f);for(int i=0;i<8;i++)assert(fscanf(f,"%u",&ticks[i])==1);fclose(f);}
 for(uint32_t i=0;i<=count;i++){
  if(captures<8 && i==ticks[captures]){
   intptr_t n=rc_game_checkpoint(h,NULL,0);assert(n>20);uint8_t*saved=malloc((size_t)n);assert(saved && rc_game_checkpoint(h,saved,(size_t)n)==n);
   char path[80];snprintf(path,sizeof(path),"%s.save",names[captures]);write_file(path,saved,(size_t)n);
   uint64_t restored=rc_game_create(scene,ns,error,sizeof(error));assert(restored && rc_game_restore_checkpoint(restored,saved,(size_t)n,error,sizeof(error))==0);
   intptr_t size=rc_game_graphics(h,1,NULL,0);assert(size>0);uint8_t*a=malloc((size_t)size),*b=malloc((size_t)size);assert(a&&b);assert(rc_game_graphics(h,1,a,(size_t)size)==size);assert(rc_game_graphics(restored,1,b,(size_t)size)==size && !memcmp(a,b,(size_t)size));
   rc_game_release(restored);free(a);free(b);free(saved);captures++;fprintf(stderr,"checkpoint %s tick=%u\n",names[captures-1],i);
  }
  if(i==count)break;
  advance(h,rows+12+(size_t)i*20);
  if(energy_weapon){int energy=rc_game_energy_status(h);assert(energy>=0 && energy<=1000);if(energy<minimum_energy)minimum_energy=energy;uint32_t status[10];assert(rc_game_status(h,status,10)==10 && status[2]==0);}
  if(i%1000==0)fprintf(stderr,"tick %u/%u\n",i,count);
 }
 uint32_t completed[10],entry[10];assert(rc_game_status(h,completed,10)==10 && completed[4]==1 && captures==8);
 free(scene);scene=read_file(argv[3],&ns);
 assert(rc_game_travel(h,scene,ns,error,sizeof(error))==0);
 for(int i=0;i<60;i++)assert(rc_game_advance(h,1./60.,0,0,0,0,0)==1);
 assert(rc_game_status(h,entry,10)==10 && entry[4]==0);
 printf("{\"passed\":true,\"ticks\":%u,\"phase\":%u,\"health\":%u,\"checkpoints\":%u,\"map_b_live_ticks\":60,\"energy_weapon\":%s,\"minimum_energy_permille\":%d}\n",count,completed[4],completed[0],captures,energy_weapon?"true":"false",energy_weapon?minimum_energy:-1);
 rc_game_release(h);free(scene);free(rows);return 0;
}
