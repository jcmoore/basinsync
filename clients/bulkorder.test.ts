import {expect,test} from "bun:test";
import {encodeBulkOrder,decodeBulkOrder,key,fromKey} from "./bulkorder";
import {encodeTuples,entryDigest} from "./events";
import {wrap,MARGIN} from "./envbuf";
import {encode} from "./flexbuffer";

const event=encodeTuples([[Symbol.species,"sync.memoremix.com","sts/0/databasin/ingestturso",{turso:{}}]]);
test("dense BulkOrder index retains nested event boundaries and arbitrary-precision lowest",()=>{
  for(const lowest of [1n,42n,65535n,1n<<256n])for(const count of [1,2,3,4,255,256]){
    const bytes=encodeBulkOrder(lowest,Array(count).fill(event));const {index,events,range}=decodeBulkOrder(bytes);
    expect(index).toEqual({lowest,offsets:Array.from({length:count+1},(_,i)=>i*event.length)});
    expect(range.start).toBe(count*event.length);expect(events.length).toBe(count);
    for(const extracted of events){expect(extracted.buffer).toBe(bytes.buffer);expect(entryDigest(extracted)).toBe(entryDigest(event));}
    expect(fromKey(key(lowest,lowest+BigInt(count)))).toEqual({lowest,highest:lowest+BigInt(count)});
  }
});
test("BulkOrder rejects malformed bounds, framing, and dense indexes",()=>{
  expect(()=>encodeBulkOrder(0n,[event])).toThrow();expect(()=>encodeBulkOrder(1n,[])).toThrow();
  for(const text of ["analogue/0yfff/0yffe","analogue/0yffe/0yffe","analogue/0z001/0z002","analogue/0yffe/0yffd/x"])expect(()=>fromKey(text)).toThrow();
  const valid=encodeBulkOrder(1n,[event]),bad=new Uint8Array(valid);bad[event.length-1]^=1;expect(()=>decodeBulkOrder(bad)).toThrow();
  for(const offsets of [[0],[1,event.length],[0,event.length+8],[0,-1],[0,1.5],[0,event.length,event.length]]){
    const bytes=new Uint8Array([...event,...wrap(encode([Symbol.species,"sync.memoremix.com","sts/0/databasin/bulkorder",{lowest:1n,offsets}])),...wrap(new Uint8Array(8),MARGIN)]);
    expect(()=>decodeBulkOrder(bytes)).toThrow();
  }
});
