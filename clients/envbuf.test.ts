import { expect,test } from "bun:test";
import { all,at,before,concat,wrap } from "./envbuf";
import { encodeWalEvent,rangesAt,eventTuples,deltasAt,type WalDelta } from "./events";
import { sha384 } from "./client";

test("default EnvBuf total lengths and concatenated boundaries are multiples of sixteen",()=>{
  const chunks=[];
  for(let length=0;length<=1024;length++){
    const content=new Uint8Array(length).fill(0x42),frame=wrap(content,[0,4,5][length%3]);
    expect(frame.length).toBe(16+Math.ceil(length/16)*16);
    expect(frame[3]&31).toBe(4);expect(at(frame,0).content).toEqual(content);chunks.push(frame);
  }
  const bytes=concat(chunks),frames=all(bytes);
  for(const frame of frames){expect(frame.start%16).toBe(0);expect(frame.end%16).toBe(0);expect(before(bytes,frame.end)).toEqual(frame);}
  expect([...wrap(new Uint8Array([1]))]).toEqual([0x65,0x6e,0x56,0xa4,1,0,0,0,1,...new Array(15).fill(0xee),1,0,0,0,0xa4,0x76,0x4e,0x45]);
});
test("EnvBuf still reads power-three frames",()=>{
  const frame=new Uint8Array([0x65,0x6e,0x56,0xa3,1,0,0,0,1,...new Array(7).fill(0xee),1,0,0,0,0xa3,0x76,0x4e,0x45]),bytes=concat([frame,frame]);
  expect(all(bytes).length).toBe(2);expect(before(bytes,48)).toEqual(at(bytes,24));expect(before(bytes,24).content).toEqual(new Uint8Array([1]));
});
test("complete EnvBuf sizes round up to the declared alignment, including framing",()=>{
  for(let power=0;power<=10;power++){
    const alignment=2**power,chunks=[];
    for(let length=0;length<=128;length++){
      const content=new Uint8Array(length).fill(0x42),frame=wrap(content,5,power);
      expect(frame.length).toBe(Math.ceil((length+16)/alignment)*alignment);
      expect(frame[3]&31).toBe(power);expect(at(frame,0).content).toEqual(content);chunks.push(frame);
    }
    const bytes=concat(chunks),frames=all(bytes);expect(frames.length).toBe(129);
    for(const frame of frames){expect((frame.end-frame.start)%alignment).toBe(0);expect(before(bytes,frame.end)).toEqual(frame);}
  }
  expect(wrap(new Uint8Array(16),5,5).length).toBe(32);
  for(const power of [-1,1.5,32,NaN])expect(()=>wrap(new Uint8Array(),5,power)).toThrow();
});
test("EnvBuf supports aligned forward/reverse walks, nested magic, and padding validation",()=>{
  const contents=[new Uint8Array(),new Uint8Array([1]),new TextEncoder().encode("enV vNE"),new Uint8Array(256).fill(0xee)];
  const bytes=concat(contents.map(c=>wrap(c))),frames=all(bytes);expect(frames.map(f=>f.content)).toEqual(contents);
  let end=bytes.length;for(const original of [...contents].reverse()){const frame=before(bytes,end);expect(frame.content).toEqual(original);expect(at(bytes,frame.start)).toEqual(frame);end=frame.start;}expect(end).toBe(0);
  const bad=wrap(new Uint8Array([1]));bad[9]=0;expect(()=>all(bad)).toThrow();
  for(let n=1;n<bytes.length;n++)if(!frames.some(f=>f.end===n))expect(()=>all(bytes.subarray(0,n))).toThrow();
});
test("independent EnvBuf event writer records absolute blob offsets across FlexBuffers width boundaries",()=>{
  const commands=[0,1,7,8,15,16,17,255,256,65535,65536].map((length,i)=>{const blob=new Uint8Array(length).fill(i);return {[`flow-${i}`]:{viewpoint:"0",sha384:sha384(blob),edges:[],context:{},blob}};});
  const bytes=encodeWalEvent(commands,{} as any),ranges=rangesAt(bytes);
  expect(eventTuples(bytes).length).toBe(commands.length+1);
  for(const range of ranges){expect(bytes.subarray(range.offset,range.offset+range.length)).toEqual(range.marker.blob);expect(sha384(range.marker.blob)).toBe(range.marker.sha384);}
  const frames=all(bytes);expect(frames.length).toBe(commands.length+2);expect(before(bytes,bytes.length)).toEqual(frames.at(-1)!);for(const frame of frames)expect((frame.end-frame.start)%16).toBe(0);
});

test("ingestion-only events have one indexed command and no marker ranges",()=>{
  const delta:WalDelta={version:"test",logical:{base:null,header:new Uint8Array(),frames:[]},physical:{format:"sqlite-wal/1",base:"0",milestone:"1",parent:null,base_sha384:null,segments:[]}};
  const bytes=encodeWalEvent([],delta);
  expect(all(bytes).length).toBe(2);
  expect(eventTuples(bytes).map(t=>t[2])).toEqual(["sts/0/databasin/ingestturso"]);
  expect(rangesAt(bytes)).toEqual([]);
  expect(deltasAt(bytes).turso["default.turso"]).toEqual(delta);
});
