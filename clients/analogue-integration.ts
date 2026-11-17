import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {GetObjectCommand,HeadObjectCommand,ListObjectsV2Command,PutObjectCommand,DeleteObjectCommand} from "@aws-sdk/client-s3";
import {fixture,eventually} from "./analogue-fixture";
import {key,fromKey,encodeBulkOrder,decodeBulkOrder} from "./bulkorder";
import {key as timelineKey,fromKey as timelineFromKey} from "./timeline";
import {rangesAt} from "./events";
import {concat} from "./envbuf";
import {commit} from "./client";

const f=await fixture();
try{
  const events=await f.generate(64,4096),bucket=await f.bucket();await f.seed(bucket,events);
  // Independent Rust/TS writers and readers, including short bigint payloads.
  for(const lowest of [1n,42n,65535n,1n<<256n]){
    const selected=events.slice(0,4),ts=encodeBulkOrder(lowest,selected);
    const rustIndex=JSON.parse(new TextDecoder().decode(await f.binary(["--decode"],{},ts)));
    const rust=await f.binary(["--pack",String(lowest)],{},concat(selected)),decoded=decodeBulkOrder(rust);
    assert.equal(decoded.index.lowest,lowest);assert.deepEqual(decoded.events,selected);assert.deepEqual(decoded.index.offsets,rustIndex.offsets);
  }
  const baseline=await f.recover(bucket,"sequential");assert.equal(baseline.milestone,"64");
  async function put(low:number,high:number,bytes=encodeBulkOrder(BigInt(low),events.slice(low-1,high-1))){const {range}=decodeBulkOrder(bytes);await f.s3.send(new PutObjectCommand({Bucket:bucket,Key:key(BigInt(low),BigInt(high)),Body:bytes,Metadata:{"bulkorder-range":`bytes=${range.start}-${range.end-1}`}}));}
  await put(1,9);await put(1,25);await put(9,41);await put(17,65);
  const calls:{method:string;url:URL}[]=[];
  const traced=f.relay(async(request,next)=>{
    const url=new URL(request.url);calls.push({method:request.method,url});const response=await next();
    if(url.searchParams.get("prefix")==="analogue/"){
      const text=await response.text(),contents=[...text.matchAll(/<Contents>.*?<\/Contents>/gs)];
      if(contents.length>1){let first=true;const paged=text.replace(/<Contents>.*?<\/Contents>/gs,match=>{if(first){first=false;return match;}return "";}).replace(/<IsTruncated>.*?<\/IsTruncated>/,"<IsTruncated>true</IsTruncated>").replace(/<KeyCount>.*?<\/KeyCount>/,"<KeyCount>1</KeyCount>");const headers=new Headers(response.headers);headers.delete("content-length");return new Response(paged,{status:response.status,headers});}
      return new Response(text,{status:response.status,headers:response.headers});
    }
    return response;
  });
  for(const mode of ["parallel","bulk","range","hedged"]){
    calls.length=0;const restored=await f.recover(bucket,mode,traced);assert.equal(restored.sha384,baseline.sha384);assert.deepEqual(restored.flows,baseline.flows);
    if(mode!=="parallel")assert(calls.some(c=>c.method==="GET"&&c.url.pathname.includes("/analogue/")),`${mode} used BulkOrders`);
    if(mode==="range")assert(calls.some(c=>c.method==="HEAD"&&c.url.pathname.includes("/analogue/")));
    const singles=calls.filter(c=>c.method==="GET"&&c.url.pathname.includes("/timeline/")).length;
    if(mode==="bulk"||mode==="range")assert(singles<events.length/2,`reduced Timeline GETs: ${singles}`);
  }
  // A stronger but corrupt candidate must not install a false prefix or poison fallback.
  const corrupt=encodeBulkOrder(1n,events),offset=decodeBulkOrder(corrupt).index.offsets[11]+rangesAt(events[11])[0].offset;corrupt[offset]^=1;
  await put(1,65,corrupt);
  for(const mode of ["bulk","range","hedged"]){const restored=await f.recover(bucket,mode);assert.equal(restored.sha384,baseline.sha384);}
  await f.s3.send(new DeleteObjectCommand({Bucket:bucket,Key:key(1n,65n)}));
  const malformed=encodeBulkOrder(1n,events);malformed[malformed.length-1]^=1;
  await f.s3.send(new PutObjectCommand({Bucket:bucket,Key:key(1n,66n),Body:malformed}));
  assert.equal((await f.recover(bucket,"bulk")).sha384,baseline.sha384);
  const denied=f.relay(async(request,next)=>new URL(request.url).pathname.includes("/analogue/")?new Response(null,{status:503}):next());
  assert.equal((await f.recover(bucket,"bulk",denied)).sha384,baseline.sha384);
  // Discovery can be arbitrarily slow without preventing direct Timeline progress.
  const gate=Promise.withResolvers<void>();let blocked=false;
  const slow=f.relay(async(request,next)=>{if(new URL(request.url).searchParams.get("prefix")==="analogue/"){blocked=true;await gate.promise;}return next();});
  const fast=await f.recover(bucket,"bulk",slow);assert.equal(fast.sha384,baseline.sha384);assert(blocked);gate.resolve();
  {
    const anchored=await f.bucket();await f.seed(anchored,events.slice(0,16));
    const app=await f.server(anchored,{BASINSYNC_BACKUP_INTERVAL_MS:"10"});
    await eventually(async()=>{const objects=await f.s3.send(new ListObjectsV2Command({Bucket:anchored,Prefix:"epilogue/"}));const marker=objects.Contents?.find(o=>o.Key!.startsWith("epilogue/0y"));return marker?f.s3.send(new HeadObjectCommand({Bucket:anchored,Key:marker.Key})):null;},head=>head?.Metadata?.milestone==="16");
    await f.stop(app.process);await f.seed(anchored,events.slice(16),17);
    const body=encodeBulkOrder(1n,events),decoded=decodeBulkOrder(body);
    await f.s3.send(new PutObjectCommand({Bucket:anchored,Key:key(1n,65n),Body:body,Metadata:{"bulkorder-range":`bytes=${decoded.range.start}-${decoded.range.end-1}`}}));
    const reads:{path:string;range:string|null}[]=[];
    const observed=f.relay(async(request,next)=>{if(request.method==="GET")reads.push({path:new URL(request.url).pathname,range:request.headers.get("range")});return next();});
    assert.equal((await f.recover(anchored,"range",observed)).sha384,baseline.sha384);
    assert(reads.some(r=>r.path.includes("/analogue/")&&r.range&&Number(r.range.split("-")[0].slice(6))>=decoded.index.offsets[16]&&Number(r.range.split("-")[0].slice(6))<decoded.range.start),"fetch a useful suffix without its checkpointed prefix");
    assert(!reads.some(r=>r.path.includes("/timeline/")&&timelineFromKey(r.path.slice(r.path.indexOf("timeline/")))<=16n),"never re-read the snapshot prefix");
    await f.s3.send(new DeleteObjectCommand({Bucket:anchored,Key:key(1n,65n)}));
    for(const [low,high] of [[1,25],[25,41],[41,57],[57,65]]){
      const body=encodeBulkOrder(BigInt(low),events.slice(low-1,high-1)),{range}=decodeBulkOrder(body);
      await f.s3.send(new PutObjectCommand({Bucket:anchored,Key:key(BigInt(low),BigInt(high)),Body:body,Metadata:{"bulkorder-range":`bytes=${range.start}-${range.end-1}`}}));
    }
    let individual=0;
    const delayed=f.relay(async(request,next)=>{const url=new URL(request.url);if(url.searchParams.get("prefix")==="analogue/")await Bun.sleep(40);if(request.method==="GET"&&url.pathname.includes("/timeline/")){individual++;await Bun.sleep(20);}return next();});
    assert.equal((await f.recover(anchored,"bulk",delayed)).sha384,baseline.sha384);
    assert(individual<12,`a speculative discovery frontier must retain orders ending behind it: ${individual} individual GETs`);
  }
  console.log("put of success: Rust/TS BulkOrder interoperability, overlapping and contained intervals, bounded recovery modes, exact images, range suffixes, corruption fallback, and nonblocking discovery");

  // Hold an archive PUT while later foreground commits fill/drop the byte handoff.
  {
    const b=await f.bucket(),entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let held=false,sources=0;
    const proxy=f.relay(async(request,next)=>{
      const path=new URL(request.url).pathname;
      if(request.method==="GET"&&path.includes("/timeline/"))sources++;
      if(request.method==="PUT"&&path.includes("/analogue/")&&!held){held=true;entered.resolve();await release.promise;}
      return next();
    });
    const app=await f.server(b,{BASINSYNC_BULK_MAX_AGE_MS:"60000",BASINSYNC_BULK_TARGET_BYTES:"1"},proxy);
    assert.equal((await commit(app.url,"live",new Uint8Array([0]),0n)).status,201);await entered.promise;
    for(let n=1;n<8;n++)assert.equal((await commit(app.url,"live",new Uint8Array([n]),BigInt(n))).status,201);
    release.resolve();await f.stop(app.process);assert(sources>0,"overflow catches up from durable Timeline");
    const orders=(await f.s3.send(new ListObjectsV2Command({Bucket:b,Prefix:"analogue/"}))).Contents!;assert.equal(orders.length,8);
    for(const object of orders){const response=await f.s3.send(new GetObjectCommand({Bucket:b,Key:object.Key!}));const bytes=await response.Body!.transformToByteArray();const {index,events,range}=decodeBulkOrder(bytes);assert.equal(events.length,1);assert.equal(response.Metadata!["bulkorder-range"],`bytes=${range.start}-${range.end-1}`);const original=await f.s3.send(new GetObjectCommand({Bucket:b,Key:timelineKey(index.lowest)}));assert.deepEqual(events[0],await original.Body!.transformToByteArray());}
    let puts=0;const observed=f.relay(async(request,next)=>{if(request.method==="PUT"&&new URL(request.url).pathname.includes("/analogue/"))puts++;return next();});
    const resumed=await f.server(b,{BASINSYNC_BULK_MAX_AGE_MS:"10",BASINSYNC_BULK_TARGET_BYTES:"1"},observed);await f.stop(resumed.process);assert.equal(puts,0,"resume existing contiguous archive coverage");
  }
  // Byte threshold, age threshold, final drain, failed uploads, and competing workers.
  {
    const b=await f.bucket(),app=await f.server(b,{BASINSYNC_BULK_MAX_AGE_MS:"60000",BASINSYNC_BULK_TARGET_BYTES:"5242880"});
    for(let n=0;n<64;n++)assert.equal((await commit(app.url,"large",new Uint8Array(98304).fill(n),BigInt(n))).status,201);
    await eventually(()=>f.s3.send(new ListObjectsV2Command({Bucket:b,Prefix:"analogue/"})),r=>(r.KeyCount??0)>0);
    await f.stop(app.process);const orders=(await f.s3.send(new ListObjectsV2Command({Bucket:b,Prefix:"analogue/"}))).Contents!;
    const sorted=orders.map(o=>({...fromKey(o.Key!),key:o.Key!})).sort((a,b)=>a.lowest<b.lowest?-1:1);let next=1n;
    for(const order of sorted){assert.equal(order.lowest,next);const response=await f.s3.send(new GetObjectCommand({Bucket:b,Key:order.key}));const decoded=decodeBulkOrder(await response.Body!.transformToByteArray());if(order.highest!==65n)assert(decoded.range.start>=5*1024*1024);next=order.highest;}
    assert.equal(next,65n);assert(sorted.length>=2);
    const restored=await f.recover(b,"bulk");assert.equal(restored.flows.large,"63");
  }
  {
    const b=await f.bucket();let fail=true;const proxy=f.relay(async(request,next)=>request.method==="PUT"&&new URL(request.url).pathname.includes("/analogue/")&&fail?new Response(null,{status:500}):next());
    const app=await f.server(b,{BASINSYNC_BULK_MAX_AGE_MS:"20"},proxy);assert.equal((await commit(app.url,"age",new Uint8Array([1]),0n)).status,201);
    await eventually(async()=>readFileSync(app.log,"utf8"),log=>log.includes("BulkOrder upload will retry"));fail=false;
    await eventually(()=>f.s3.send(new HeadObjectCommand({Bucket:b,Key:key(1n,2n)})),r=>!!r.ContentLength);
    await f.stop(app.process);
    const source=(await f.s3.send(new GetObjectCommand({Bucket:b,Key:timelineKey(1n)})));const event=await source.Body!.transformToByteArray();
    // New workers can overlap; one conditional winner supplies the same event identities.
    const other=await f.bucket();await f.s3.send(new PutObjectCommand({Bucket:other,Key:timelineKey(1n),Body:event}));
    const apps=await Promise.all([f.server(other,{BASINSYNC_BULK_MAX_AGE_MS:"10"}),f.server(other,{BASINSYNC_BULK_MAX_AGE_MS:"10"})]);await Promise.all(apps.map(app=>f.stop(app.process)));
    assert.equal((await f.s3.send(new ListObjectsV2Command({Bucket:other,Prefix:"analogue/"}))).KeyCount,1);
  }
  console.log(`put of success: byte/age/shutdown flushing, nonblocking bounded handoff, retry, restart, immutable races, and exact archived bytes. Artifacts: ${f.artifacts}`);
}finally{await f.close();}
