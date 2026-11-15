import assert from "node:assert/strict";
import {createServer} from "node:net";
import {mkdtempSync,openSync,closeSync,readFileSync} from "node:fs";
import {S3Client,CreateBucketCommand,ListMultipartUploadsCommand,HeadObjectCommand} from "@aws-sdk/client-s3";
import {commit,reserveUserHeap,parseResponse,form,epilogueStatus,type UserHeapResponse} from "./client";
import {s3Fixture} from "./s3-fixture";

const root=new URL("../",import.meta.url).pathname;
const artifacts=mkdtempSync(`${root}.local/cancellation-probe-`);
const processes:ReturnType<typeof Bun.spawn>[]=[];
const proxies:ReturnType<typeof Bun.serve>[]=[];
const credentials={accessKeyId:"audit-test",secretAccessKey:"audit-test-secret"};
const bytes=(s:string)=>new TextEncoder().encode(s);
async function port(){const s=createServer();await new Promise<void>(r=>s.listen(0,"127.0.0.1",r));const p=(s.address() as {port:number}).port;await new Promise<void>(r=>s.close(()=>r()));return p;}
async function deadline<T>(p:Promise<T>,ms=15000){let timer:ReturnType<typeof setTimeout>;try{return await Promise.race([p,new Promise<never>((_,no)=>{timer=setTimeout(()=>no(Error("deadline")),ms);})]);}finally{clearTimeout(timer!);}}
async function until<T>(get:()=>Promise<T>,ok:(t:T)=>boolean,ms=15000){const end=Date.now()+ms;while(Date.now()<end){try{const v=await get();if(ok(v))return v;}catch{}await Bun.sleep(20);}throw Error("until deadline");}
function launch(command:string[],env:Record<string,string>){const path=`${artifacts}/${processes.length}.log`,fd=openSync(path,"w");const process=Bun.spawn(command,{cwd:root,env:{...Bun.env,...env},stdout:fd,stderr:fd});closeSync(fd);processes.push(process);return {process,path};}
async function stop(p:ReturnType<typeof Bun.spawn>){if(p.exitCode!==null)return;p.kill("SIGINT");try{await deadline(p.exited,2000);}catch{p.kill("SIGKILL");await p.exited;}}
const endpoint=`http://127.0.0.1:${await port()}`;
const s3=new S3Client({endpoint,region:"us-east-1",forcePathStyle:true,credentials,maxAttempts:1});
let seq=0;
async function bucket(){const Bucket=`audit-${++seq}`;await s3.send(new CreateBucketCommand({Bucket}));return Bucket;}
async function app(Bucket:string,s3Endpoint:string,options:Record<string,string>={}){
  const address=`127.0.0.1:${await port()}`;
  const p=launch([`${root}target/debug/basinsync`],{S3_ENDPOINT:s3Endpoint,S3_BUCKET:Bucket,AWS_REGION:"us-east-1",AWS_ACCESS_KEY_ID:credentials.accessKeyId,AWS_SECRET_ACCESS_KEY:credentials.secretAccessKey,AWS_SESSION_TOKEN:"",BASINSYNC_LISTEN:address,BASINSYNC_CLEANUP_INTERVAL_MS:"0",BASINSYNC_BACKUP_INTERVAL_MS:"0",BASINSYNC_BATCH_MAX:"1",BASINSYNC_BATCH_WAIT_MS:"0",RUST_LOG:"basinsync=debug",...options});
  const url=`http://${address}`;await until(async()=>{if(p.process.exitCode!==null)throw Error(readFileSync(p.path,"utf8"));return (await fetch(`${url}/ready`)).status;},s=>s===404);return {...p,url};
}
function proxy(before:(request:Request)=>Promise<Response|void>,after:((request:Request,response:Response)=>Promise<void>)=async()=>{}){
  const server=Bun.serve({hostname:"127.0.0.1",port:0,async fetch(request){const early=await before(request);if(early)return early;const url=new URL(request.url);const response=await fetch(`${endpoint}${url.pathname}${url.search}`,{method:request.method,headers:request.headers,body:["GET","HEAD"].includes(request.method)?undefined:await request.arrayBuffer(),redirect:"manual"});await after(request,response);return response;}});proxies.push(server);return `http://127.0.0.1:${server.port}`;
}
try{
  const fixture=s3Fixture(root,endpoint,`${artifacts}/s3`,credentials);
  launch(fixture.command,fixture.env);
  await until(async()=>{await s3.send(new CreateBucketCommand({Bucket:"readiness"}));return true;},Boolean);
  // A source error must await multipart abort, preserve the source, and allow retry.
  for(const fault of ["source","upload"]){
    const b=await bucket(),part=Promise.withResolvers<void>(),secondRange=Promise.withResolvers<void>(),failed=Promise.withResolvers<void>(),aborting=Promise.withResolvers<void>(),releaseAbort=Promise.withResolvers<void>();
    let initiations=0,parts=0,aborts=0,failures=0,uploadId="",uploadKey="";
    const p=proxy(async request=>{
      const u=new URL(request.url);
      if(request.method==="PUT"&&u.pathname.includes("/userheap/old/")&&request.headers.has("x-amz-copy-source"))return new Response("unsupported",{status:501});
      if(request.method==="GET"&&u.pathname.includes("/userheap/young/")&&request.headers.get("range")?.startsWith("bytes=8388608-")){
        secondRange.resolve();
        if(fault==="source"){await part.promise;failures++;failed.resolve();return new Response("source read failed",{status:500});}
      }
      if(fault==="upload"&&request.method==="PUT"&&u.pathname.includes("/userheap/old/")&&u.searchParams.has("partNumber")){await secondRange.promise;failed.resolve();return new Response("part failed",{status:500});}
      if(request.method==="DELETE"&&u.searchParams.has("uploadId")){aborts++;aborting.resolve();await releaseAbort.promise;}
    },async(request,response)=>{
      const u=new URL(request.url);
      if(request.method==="POST"&&u.pathname.includes("/userheap/old/")&&u.searchParams.has("uploads")&&response.ok){initiations++;uploadKey=decodeURIComponent(u.pathname.slice(b.length+2));const xml=await response.clone().text();uploadId=xml.match(/<UploadId>([^<]+)/)?.[1]??"";}
      if(request.method==="PUT"&&u.pathname.includes("/userheap/old/")&&u.searchParams.has("partNumber")&&response.ok){parts++;part.resolve();}
    });
    const server=await app(b,p,{BASINSYNC_CLEANUP_INTERVAL_MS:"1000",BASINSYNC_PROMOTION_ALLOW_TRANSFER:"true"});
    const data=new Uint8Array(9*1024*1024+3).fill(42),node={auth:[0,0,bytes("multipart-audit-token")] as [number,number,Uint8Array],length:data.length,edges:[]};
    const reservation=await reserveUserHeap(server.url,"f",0n,[node]);assert.equal(reservation.status,200);const actions=(await parseResponse<UserHeapResponse>(reservation)).results![0].actions;
    const put=await fetch(actions.write!.url,{method:"PUT",headers:actions.write!.headers,body:data});assert(put.ok);
    assert.equal((await commit(server.url,"f",bytes("marker"),0n,undefined,[{auth:node.auth}])).status,201);
    await deadline(failed.promise);await deadline(aborting.promise);assert.equal(initiations,1);assert.equal(parts,fault==="source"?1:0);assert.equal(aborts,1);assert(uploadId);
    server.process.kill("SIGINT");await Bun.sleep(100);assert.equal(server.process.exitCode,null,"shutdown awaits abort cleanup");
    releaseAbort.resolve();assert.equal(await deadline(server.process.exited),0);
    assert.equal((await s3.send(new ListMultipartUploadsCommand({Bucket:b}))).Uploads?.length??0,0);
    await assert.rejects(s3.send(new HeadObjectCommand({Bucket:b,Key:uploadKey})),"a short destination must never be completed");
    const source=await fetch(actions.preview!.url,{headers:actions.preview!.headers});assert.equal(source.status,200);assert.equal((await source.arrayBuffer()).byteLength,data.length);
    const restored=await app(b,endpoint,{BASINSYNC_CLEANUP_INTERVAL_MS:"10"});
    await until(async()=>{const r=await fetch(actions.read.url,{headers:actions.read.headers});const body=new Uint8Array(await r.arrayBuffer());return r.status===200&&body.length===data.length;},Boolean);
    await stop(restored.process);
    console.log(`put of success: ${fault} failure awaits multipart abort, cannot publish a short object, and retries after restart`);
  }
  // Disconnected requests still drain, including a prepared successor batch.
  for(const signal of (process.platform==="win32"?["SIGINT"]:["SIGINT","SIGTERM"]) as ("SIGINT"|"SIGTERM")[]){
    const b=await bucket(),entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let puts=0;
    const p=proxy(async request=>{if(request.method==="PUT"&&new URL(request.url).pathname.includes("/timeline/")){if(++puts===1){entered.resolve();await release.promise;}}});
    const server=await app(b,p),abort=new AbortController();
    const request=fetch(`${server.url}/api/v0/epilogue`,{method:"PUT",body:form("disconnect",bytes("marker"),0n),signal:abort.signal}).then(()=>"response",()=>"disconnected");
    await deadline(entered.promise);abort.abort();assert.equal(await request,"disconnected");
    const successor=commit(server.url,"disconnect",bytes("next marker"),1n);
    await until(async()=>readFileSync(server.path,"utf8"),s=>s.includes("milestone=2")&&s.includes("batch prepared"));
    server.process.kill(signal);await Bun.sleep(100);assert.equal(server.process.exitCode,null,"shutdown waits for admitted work");
    release.resolve();assert.equal((await successor).status,201);assert.equal(await deadline(server.process.exited),0);assert.equal(puts,2);
    const restored=await app(b,endpoint);assert.equal((await epilogueStatus(restored.url,"disconnect")).latest?.viewpoint,1n);await stop(restored.process);
    console.log(`put of success: ${signal} drains disconnected and pipelined commits in order and recovery sees both`);
  }
  // Closing the publisher wakes even a long backup interval and flushes its final target.
  {
    const b=await bucket(),entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();let backups=0;
    const p=proxy(async request=>{if(request.method==="PUT"&&new URL(request.url).pathname.includes(`/${b}/epilogue/`)&&request.headers.has("x-amz-meta-milestone")){backups++;entered.resolve();await release.promise;}});
    const server=await app(b,p,{BASINSYNC_BACKUP_INTERVAL_MS:"3600000"});
    assert.equal((await commit(server.url,"backup",bytes("marker"),0n)).status,201);
    server.process.kill("SIGINT");await deadline(entered.promise);await Bun.sleep(50);assert.equal(server.process.exitCode,null);
    release.resolve();assert.equal(await deadline(server.process.exited),0);assert.equal(backups,1);
    console.log("put of success: shutdown awaits final backup without waiting for its periodic interval");
  }
  // The drain deadline is explicit and exits unsuccessfully rather than claiming completion.
  {
    const b=await bucket(),entered=Promise.withResolvers<void>(),release=Promise.withResolvers<void>();
    const p=proxy(async request=>{if(request.method==="PUT"&&new URL(request.url).pathname.includes("/timeline/")){entered.resolve();await release.promise;return new Response("not stored",{status:503});}});
    const server=await app(b,p,{BASINSYNC_SHUTDOWN_TIMEOUT_MS:"100"}),abort=new AbortController();
    const request=fetch(`${server.url}/api/v0/epilogue`,{method:"PUT",body:form("timeout",bytes("marker"),0n),signal:abort.signal}).catch(()=>null);
    await deadline(entered.promise);abort.abort();await request;server.process.kill("SIGINT");
    assert.equal(await deadline(server.process.exited),1);release.resolve();assert(readFileSync(server.path,"utf8").includes("shutdown drain timed out"));
    console.log("put of success: exhausted shutdown deadline reports failure with an uncertain in-flight outcome");
  }
  console.log(`Artifacts: ${artifacts}`);
}finally{
  await Promise.all(processes.map(stop));await Promise.all(proxies.map(p=>p.stop(true)));s3.destroy();
}
