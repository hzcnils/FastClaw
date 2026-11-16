// Read-only trace input; write only the derived report explicitly named by the caller.
import { readFileSync, writeFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';

const [input, output] = process.argv.slice(2);
assert(input && output && input !== output, 'Usage: node script.mjs input.json.gz output.json');
const raw = readFileSync(input);
const trace = JSON.parse(gunzipSync(raw));
const events = trace.traceEvents;
const main = events.find(e => e.name === 'thread_name' && e.args?.name === 'CrRendererMain');
assert(main, 'Renderer main thread required');
const es = events.filter(e => e.pid === main.pid && e.tid === main.tid && e.ts > 0);
const start = es.reduce((min, e) => Math.min(min, e.ts), Infinity);
const ms = n => Math.round(n / 1000 * 100) / 100;
const time = ts => ms(ts - start);
const count = new Map();
for (const e of es) count.set(e.name, (count.get(e.name) || 0) + 1);
const counters = es.filter(e => e.name === 'UpdateCounters');
const counterPoint = e => ({ atMs: time(e.ts), ...e.args.data });
const ranges = {};
for (const key of ['jsHeapSizeUsed', 'nodes', 'jsEventListeners', 'documents']) {
  const values = counters.filter(e => Number.isFinite(e.args?.data?.[key]));
  const sorted = [...values].sort((a,b) => a.args.data[key] - b.args.data[key]);
  ranges[key] = { first: counterPoint(values[0]), last: counterPoint(values.at(-1)), min: counterPoint(sorted[0]), max: counterPoint(sorted.at(-1)) };
}
const tasks = es.filter(e => e.name === 'RunTask' && e.ph === 'X' && e.dur >= 50000);
const sortedTasks = [...tasks].sort((a,b) => a.ts-b.ts);
for (let i=1;i<sortedTasks.length;i++) assert(sortedTasks[i].ts >= sortedTasks[i-1].ts+sortedTasks[i-1].dur, 'Do not double-count nested RunTask intervals');
const longest = [...tasks].sort((a,b) => b.dur-a.dur).slice(0, 20);
const self = new Map(), inclusive = new Map(), app = new Map();
const add = (map, key, dt) => map.set(key, (map.get(key) || 0) + dt);
const profileReports = [];
const errorStacks = [];
for (const profile of es.filter(e => e.name === 'Profile')) {
  const chunks = events.filter(e => e.name === 'ProfileChunk' && e.pid === profile.pid && e.id === profile.id).sort((a,b) => a.ts-b.ts);
  const nodes = new Map();
  for (const c of chunks) for (const node of c.args.data.cpuProfile?.nodes || []) nodes.set(node.id, node);
  let ts = profile.args.data.startTime, negative = 0;
  const points = [];
  for (const c of chunks) {
    const ids = c.args.data.cpuProfile?.samples || [], ds = c.args.data.timeDeltas || [];
    assert.equal(ids.length, ds.length);
    for (let i=0;i<ids.length;i++) { ts += ds[i]; if(ds[i]<0) negative++; points.push({ts, id:ids[i]}); }
  }
  points.sort((a,b)=>a.ts-b.ts);
  const chains = new Map();
  for (let i=0;i<points.length-1;i++) {
    const p=points[i], dt=points[i+1].ts-p.ts;
    if (!chains.has(p.id)) {
      let node=nodes.get(p.id); const seen=new Set(), chain=[];
      while(node && !seen.has(node.id)) {
        seen.add(node.id); const f=node.callFrame;
        chain.push(JSON.stringify({name:f.functionName || '(anonymous)',url:f.url,line:(f.lineNumber??-1)+1}));
        node=nodes.get(node.parent);
      }
      chains.set(p.id, chain);
    }
    const chain=chains.get(p.id);
    if (!chain.length) continue;
    add(self,chain[0],dt);
    for(const frame of new Set(chain)) add(inclusive,frame,dt);
    const own=chain.find(k=>/\/src\/|\/chat-plan-packages\//.test(k) && !/node_modules/.test(k));
    if(own) add(app,own,dt);
  }
  profileReports.push({samples:points.length,negativeDeltas:negative,startMs:time(points[0].ts),endMs:time(points.at(-1).ts)});
  for (const error of es.filter(e => /V8Console::(Error|Warn)/.test(e.name))) {
    let lo=0,hi=points.length;
    while(lo<hi) { const mid=(lo+hi)>>>1; if(points[mid].ts<error.ts)lo=mid+1;else hi=mid; }
    errorStacks.push({atMs:time(error.ts),name:error.name,nearby:points.slice(Math.max(0,lo-2),lo+2).map(p=>({offsetUs:p.ts-error.ts,stack:(chains.get(p.id)||[]).map(f=>JSON.parse(f))}))});
  }
}
const top = (map,limit=40) => [...map].sort((a,b)=>b[1]-a[1]).slice(0,limit).map(([key,us])=>({...JSON.parse(key),ms:ms(us)}));
const report={
  input,sha256:createHash('sha256').update(raw).digest('hex'),events:events.length,main:{pid:main.pid,tid:main.tid},
  sourceMapsPresent:!!trace.metadata?.sourceMaps?.length,
  durationMs:time(es.reduce((max,e)=>Math.max(max,e.ts+(e.dur||0)),start)),
  topCounts:[...count].sort((a,b)=>b[1]-a[1]).slice(0,30), counters:ranges,
  longTasks:{count:tasks.length,totalMs:ms(tasks.reduce((s,e)=>s+e.dur,0)),top:longest.map(e=>({atMs:time(e.ts),durationMs:ms(e.dur)}))},
  profiles:profileReports,self:top(self),inclusive:top(inclusive),nearestApplication:top(app),
  errorStacks,
  clicks:es.filter(e=>e.name==='EventDispatch'&&e.args?.data?.type==='click').map(e=>({atMs:time(e.ts),durationMs:ms(e.dur||0)})),
  diagnosticEvents:es.filter(e=>/Exception|Console|Error|chat-switch|chatSwitch/.test(e.name)).slice(0,60).map(e=>({atMs:time(e.ts),name:e.name,args:e.args})),
};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report,null,2));
