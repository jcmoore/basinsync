import assert from "node:assert/strict";
import { all, before, concat, wrap, nestedMessage, NESTED, MARGIN } from "./envbuf";
import { encode, toReference, VT_VECTOR_UINT } from "./flexbuffer";
import { encode as lex, decode as unlex } from "./lexidecimal";

export const bulkmanifestPath="sts/0/databasin/bulkmanifest";
export type BulkIndex={lowest:bigint;offsets:number[]};
export function key(lowest:bigint,highest:bigint,range:{start:number;end:number}):string {
  assert(lowest>0n&&highest>lowest);assert(Number.isSafeInteger(range.start)&&Number.isSafeInteger(range.end)&&range.start>=0&&range.end>range.start&&range.start%8===0&&range.end%8===0);const key=`analogue/${lex(~lowest)}/${lex(~highest)}/bytes=${range.start}-${range.end-1}`;assert(key.length<=1024);return key;
}
export function fromKey(keyText:string):{lowest:bigint;highest:bigint;range:{start:number;end:number}} {
  const parts=keyText.split("/");assert(parts.length===4&&parts[0]==="analogue");
  const bounds=parts[3].match(/^bytes=(\d+)-(\d+)$/);assert(bounds);const range={start:Number(bounds[1]),end:Number(bounds[2])+1};const lowest=~unlex(parts[1]),highest=~unlex(parts[2]);assert.equal(key(lowest,highest,range),keyText);return {lowest,highest,range};
}
export function encodeBulkOrder(lowest:bigint,events:Uint8Array[]):Uint8Array {
  assert(lowest>0n&&events.length>0);const offsets=[0];
  for(const event of events){nestedMessage(event);offsets.push(offsets.at(-1)!+event.length);}
  const footer=wrap(encode([Symbol.species,"sync.memoremix.com",bulkmanifestPath,{lowest,offsets}]));
  return concat([...events,footer,wrap(new Uint8Array(8),MARGIN)]);
}
export function decodeBulkOrder(bytes:Uint8Array):{index:BulkIndex;events:Uint8Array[];range:{start:number;end:number}} {
  const frames=all(bytes),readable=frames.filter(e=>e.format!==MARGIN),footer=readable.pop()!;
  assert.equal(footer.format,5);const tuple=toReference(new Uint8Array(footer.content).buffer);
  assert.equal(tuple.length(),4);assert.equal(tuple.get(0).symbolValue(),Symbol.species);assert.equal(tuple.get(1).stringValue(),"sync.memoremix.com");assert.equal(tuple.get(2).stringValue(),bulkmanifestPath);
  assert.notEqual((tuple.get(3).get("offsets") as any).valueType,VT_VECTOR_UINT);
  const index=tuple.get(3).toObject() as BulkIndex;
  assert.deepEqual(Object.keys(index).sort(),["lowest","offsets"]);
  assert(typeof index.lowest==="bigint"&&index.lowest>0n&&index.offsets.length>=2&&index.offsets[0]===0);
  assert(index.offsets.every((n,i)=>Number.isSafeInteger(n)&&n%8===0&&(i===0||n>index.offsets[i-1])));
  assert.equal(index.offsets.at(-1),footer.start);assert.equal(readable.length,index.offsets.length-1);
  const events=readable.map((frame,i)=>{assert.equal(frame.format,NESTED);assert.equal(frame.start,index.offsets[i]);assert.equal(frame.end,index.offsets[i+1]);return bytes.subarray(frame.start,frame.end);});
  assert(frames.slice(readable.length+1).every(frame=>frame.format===MARGIN));
  assert.equal(before(bytes,footer.end).format,5);
  return {index,events,range:{start:footer.start,end:footer.end}};
}
