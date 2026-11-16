import { expect,test } from "bun:test";
import { all,at,before,concat,wrap,readable,MARGIN,PLATFORM_EXTENSION, message } from "./envbuf";
import { encodeWalEvent,encodeTuples,manifestPath,entryDigest,rangesAt,eventTuples,deltasAt,type WalDelta } from "./events";
import { encode,toObject } from "./flexbuffer";
import { encode as lex } from "./lexidecimal";
import { sha384 } from "./client";

test("default EnvBuf total lengths and concatenated boundaries are multiples of sixteen",()=>{
  const chunks=[];
  for(let length=0;length<=1024;length++){
    const content=new Uint8Array(length).fill(0x42),frame=wrap(content,[0,4,5,MARGIN][length%4]);
    expect(frame.length).toBe(16+Math.ceil(length/16)*16);
    expect(frame[3]&31).toBe(4);expect(at(frame,0).content).toEqual(content);chunks.push(frame);
  }
  const bytes=concat(chunks),frames=all(bytes);
  for(const frame of frames){expect(frame.start%16).toBe(0);expect(frame.end%16).toBe(0);expect(before(bytes,frame.end)).toEqual(frame);}
  expect([...wrap(new Uint8Array([1]))]).toEqual([0x65,0x6e,0x56,0xa4,1,0,0,0,1,...new Array(15).fill(0xee),1,0,0,0,0xa4,0x76,0x4e,0x45]);
});
test("margin is skipped, raw payloads remain readable, and platform extensions stay reserved",()=>{
  const ignored=wrap(new TextEncoder().encode("ignored enV vNE"),MARGIN),raw=wrap(new TextEncoder().encode("read me"),0);
  const bytes=concat([ignored,raw,wrap(new Uint8Array(),MARGIN)]),frames=all(bytes);
  expect(frames.map(f=>f.format)).toEqual([6,0,6]);expect(before(bytes,bytes.length).format).toBe(MARGIN);
  expect(readable(bytes)).toEqual([at(bytes,ignored.length)]);expect(readable(bytes)[0].content).toEqual(new TextEncoder().encode("read me"));
  const bad=new Uint8Array(bytes);bad[ignored.length-1]^=1;expect(()=>readable(bad)).toThrow();
  for(const format of [1,2,3,PLATFORM_EXTENSION])expect(()=>wrap(new Uint8Array(),format)).toThrow();
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
test("range manifests index each consecutive margin run once at its exclusive end",()=>{
  const delta:WalDelta={version:"test",logical:{base:null,header:new Uint8Array(),frames:[]},physical:{format:"sqlite-wal/1",base:"0",milestone:"1",parent:null,base_sha384:null,segments:[]}};
  const marker={viewpoint:"0",sha384:sha384(new Uint8Array([1])),blob:new Uint8Array([1]),edges:[],context:{}};
  const plain=encodeWalEvent([{flow:marker}],delta),commands=eventTuples(plain),pad={margin:new Uint8Array([255,0,1])};
  const bytes=encodeTuples([pad,pad,commands[0],pad,pad,commands[1],pad,pad]);
  const frames=all(bytes),manifest=frames.at(-1)!,index=(toObject(new Uint8Array(manifest.content)) as any[])[3].envelopes;
  const marginEnds=[frames[1].end,frames[4].end,frames[7].end].map(n=>lex(BigInt(n)));
  expect(Object.keys(index).filter(key=>index[key]===null)).toEqual(marginEnds);
  expect(Object.keys(index).length).toBe(5);expect(index[lex(BigInt(frames[0].end))]).toBeUndefined();
  const padded=concat([bytes,wrap(new Uint8Array(),MARGIN)]);
  expect(eventTuples(padded)).toEqual(commands);expect(deltasAt(padded).turso["default.turso"]).toEqual(delta);
  const ranges=rangesAt(padded);expect(ranges.length).toBe(1);expect(ranges[0].marker).toEqual(marker);
  expect(padded.subarray(ranges[0].offset,ranges[0].offset+ranges[0].length)).toEqual(marker.blob);
  const prefix=commands[0].slice(0,3);
  for(const fault of [
    (i:Record<string,unknown>)=>{delete i[marginEnds[0]];},
    (i:Record<string,unknown>)=>{i[lex(BigInt(frames[0].end))]=null;},
    (i:Record<string,unknown>)=>{i[marginEnds[0]]=prefix;},
    (i:Record<string,unknown>)=>{delete i[marginEnds[0]];i[lex(BigInt(frames[1].end+1))]=null;},
    (i:Record<string,unknown>)=>{i[lex(BigInt(frames[2].end))]=null;},
  ]){
    const changed={...index};fault(changed);
    const bad=concat([bytes.subarray(0,manifest.start),wrap(encode([Symbol.species,"sync.memoremix.com",manifestPath,{envelopes:changed}]))]);
    expect(()=>rangesAt(bad)).toThrow();
  }
  const bad=new Uint8Array(padded);bad[frames[0].end-1]^=1;expect(()=>rangesAt(bad)).toThrow();
});

test("empty raw messages and empty margin are framed above the tombstone floor",()=>{
  const margin=wrap(new Uint8Array(),MARGIN);expect(margin.length).toBe(16);expect(all(margin)[0].content.length).toBe(0);
  for(const length of [0,1,15,16,17,65536]){
    const raw=new Uint8Array(length).fill(42),encoded=wrap(raw,0);expect(message(encoded)).toEqual(raw);expect(encoded.length).toBeGreaterThanOrEqual(16);
    expect(message(concat([encoded,margin]))).toEqual(raw);
  }
});

test("Timeline identity excludes trailing retry margin while manifest ranges stay fixed",()=>{
  const commands=[[Symbol.species,"sync.memoremix.com","sts/0/databasin/ingestturso",{turso:{}}]];
  const bytes=encodeTuples(commands),identity=entryDigest(bytes),last=before(bytes,bytes.length);
  for(const counter of [0n,1n,256n,(1n<<64n)-1n]){
    const message=new Uint8Array(8);new DataView(message.buffer).setBigUint64(0,counter,true);
    const retried=concat([bytes,wrap(message,MARGIN)]);expect(retried.length).toBe(bytes.length+32);
    expect(entryDigest(retried)).toBe(identity);expect(sha384(retried)).not.toBe(identity);
    expect(all(retried).filter(e=>e.format!==MARGIN).at(-1)!.start).toBe(last.start);
  }
});
