// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 jhq223 and contributors.
struct Params { blocks:u32, alpha:u32, pad0:u32, pad1:u32 }
@group(0) @binding(0) var<storage,read> pixels:array<u32>;
@group(0) @binding(1) var<storage,read> originals:array<u32>;
@group(0) @binding(2) var<storage,read_write> results:array<u32>;
@group(0) @binding(3) var<uniform> params:Params;
var<workgroup> errors:array<u32,64>;
var<workgroup> orders:array<u32,64>;
var<workgroup> pairs:array<u32,64>;
var<workgroup> selections:array<u32,64>;
fn expand(v:u32)->vec3<i32> { let r=(v>>11)&31u;let g=(v>>5)&63u;let b=v&31u;return vec3<i32>(i32((r<<3)|(r>>2)),i32((g<<2)|(g>>4)),i32((b<<3)|(b>>2))); }
fn neighbor(v:u32,index:u32)->u32 {
 let b=i32(v&31u)+i32(index%3u)-1;let g=i32((v>>5)&63u)+i32((index/3u)%3u)-1;let r=i32((v>>11)&31u)+i32(index/9u)-1;
 if b<0 || b>31 || g<0 || g>63 || r<0 || r>31 {return 0xffffffffu;}
 return u32((r<<11)|(g<<5)|b);
}
fn evaluate(block:u32,pair:u32,fixed:bool,old:u32)->vec2<u32> {
 let a=pair&65535u;let b=pair>>16;var pal:array<vec3<i32>,4>;pal[0]=expand(a);pal[1]=expand(b);
 var n=3u;
 if params.alpha!=0u || a>b {pal[2]=(2*pal[0]+pal[1])/3;pal[3]=(pal[0]+2*pal[1])/3;n=4u;}else{pal[2]=(pal[0]+pal[1])/2;pal[3]=vec3<i32>(0);}
 var error=0u;var selectors=0u;
 for(var p=0u;p<16u;p++) {
  let v=pixels[block*16u+p];let rgb=vec3<i32>(i32(v&255u),i32((v>>8)&255u),i32((v>>16)&255u));var best=0xffffffffu;var chosen=0u;
  if fixed {chosen=(old>>(p*2u))&3u;let d=rgb-pal[chosen];best=u32(d.x*d.x+d.y*d.y+d.z*d.z);}
  else {for(var s=0u;s<n;s++){let d=rgb-pal[s];let e=u32(d.x*d.x+d.y*d.y+d.z*d.z);if e<best {best=e;chosen=s;}}}
  error+=best;selectors|=chosen<<(p*2u);
 }
 return vec2<u32>(error,selectors);
}
@compute @workgroup_size(64)
fn main(@builtin(workgroup_id) group:vec3<u32>,@builtin(local_invocation_index) lane:u32) {
 let block=group.x;let original=originals[block*2u];let old=originals[block*2u+1u];
 let baseline=evaluate(block,original,true,old);var best=baseline.x;var order=0u;var pair=original;var selectors=old;
 for(var c=lane;c<729u;c+=64u) {
  let a=neighbor(original&65535u,c%27u);let b=neighbor(original>>16,c/27u);
  if a==0xffffffffu || b==0xffffffffu || (params.alpha!=0u && a<=b) {continue;}
  let candidate=a|(b<<16);let score=evaluate(block,candidate,false,0u);
  if score.x<best || (score.x==best && c+1u<order) {best=score.x;order=c+1u;pair=candidate;selectors=score.y;}
 }
 errors[lane]=best;orders[lane]=order;pairs[lane]=pair;selections[lane]=selectors;workgroupBarrier();
 for(var step=32u;step>0u;step/=2u) {
  if lane<step {let other=lane+step;if errors[other]<errors[lane] || (errors[other]==errors[lane] && orders[other]<orders[lane]) {errors[lane]=errors[other];orders[lane]=orders[other];pairs[lane]=pairs[other];selections[lane]=selections[other];}}
  workgroupBarrier();
 }
 if lane==0u {results[block*2u]=pairs[0];results[block*2u+1u]=selections[0];}
}
