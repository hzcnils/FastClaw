// Local R0 trace analysis. Never writes or decompresses the original recordings on disk.
import { readFileSync, writeFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { TraceMap, originalPositionFor } = require('@jridgewell/trace-mapping');
const root = new URL('../', import.meta.url);
const round = n => Math.round(n * 1000) / 1000;
const median = a => [...a].sort((a, b) => a - b)[Math.floor(a.length / 2)];
function unionLength(intervals) {
  let total = 0, end = -Infinity;
  for (const [a, b] of intervals.sort((a, b) => a[0] - b[0])) {
    if (b > end) { total += b - Math.max(a, end); end = b; }
  }
  return total;
}
assert.equal(unionLength([[0, 10], [3, 6], [8, 14], [20, 22]]), 16);

// CPU traces can arrive out of timestamp order. Keep negative deltas when
// reconstructing absolute timestamps, then sort, as DevTools does.
function sampleSpans(points) {
  points.sort((a, b) => a.time - b.time);
  return points.slice(0, -1).map((p, i) => ({ ...p, end: points[i + 1].time }));
}
assert.deepEqual(sampleSpans([{ id: 1, time: 10 }, { id: 2, time: 8 }, { id: 3, time: 12 }]),
  [{ id: 2, time: 8, end: 10 }, { id: 1, time: 10, end: 12 }]);
assert.equal(sampleSpans([{ time: 1 }, { time: 1 }, { time: 2 }]).reduce((s, p) => s + p.end - p.time, 0), 1);
const companionMaps = new Map();

function analyze(n) {
  const prefix = `docs/analysis/ChatView-R0-${n}tab-trace`;
  const raw = readFileSync(new URL(`${prefix}-20260922.json.gz`, root));
  const trace = JSON.parse(gunzipSync(raw));
  const samplesRaw = readFileSync(new URL(`${prefix}-samples-20260922.json`, root));
  const samples = JSON.parse(samplesRaw).samples;
  const events = trace.traceEvents;
  const main = events.find(e => e.name === 'thread_name' && e.args.name === 'CrRendererMain');
  assert(main, 'Renderer main thread required');
  const onMain = e => e.pid === main.pid && e.tid === main.tid;
  const timing = events.filter(e => onMain(e) && e.name === 'EventTiming' && e.ph === 'b' && Number.isFinite(e.args?.data?.timeStamp));
  const offsets = timing.map(e => e.ts - e.args.data.timeStamp * 1000);
  assert(offsets.length > 0, 'EventTiming clock anchor required');
  const offset = median(offsets);
  assert(Math.max(...offsets) - Math.min(...offsets) < 100, 'Inconsistent trace/performance.now clocks');
  const windows = samples.map(s => [offset + s.startedAt * 1000, offset + s.firstContentAt * 1000]);
  for (const s of samples) {
    assert(s.startedAt <= s.routeCommittedAt && s.routeCommittedAt <= s.viewActivatedAt && s.viewActivatedAt <= s.firstContentAt, 'Invalid switch timestamps');
    assert(Math.abs(s.firstContentAt - s.startedAt - s.clickToFirstContentMs) < 0.001, 'Inconsistent total duration');
  }
  assert.equal(unionLength(windows.map(w => [...w])), windows.reduce((s, [a, b]) => s + b - a, 0), 'Overlapping switch windows');
  const clicks = events.filter(e => onMain(e) && e.name === 'EventDispatch' && e.args?.data?.type === 'click');
  const mainEvents = events.filter(e => onMain(e) && e.ph === 'X' && e.dur > 0);
  const screenshots = events.filter(e => e.name === 'Screenshot').sort((a, b) => a.ts - b.ts);
  const overlap = (a, b, windows) => windows.reduce((sum, [lo, hi]) => sum + Math.max(0, Math.min(hi, b) - Math.max(lo, a)), 0);
  function durationFor(names, ws = windows) {
    const intervals = [];
    for (const e of mainEvents) if (names.has(e.name)) for (const [lo, hi] of ws) {
      const a = Math.max(lo, e.ts), b = Math.min(hi, e.ts + e.dur);
      if (b > a) intervals.push([a, b]);
    }
    return round(unionLength(intervals) / 1000);
  }
  const categories = {
    mainBusy: new Set(['RunTask']),
    jsEnvelope: new Set(['RunMicrotasks', 'FunctionCall', 'EvaluateScript']),
    style: new Set(['UpdateLayoutTree', 'RecalculateStyles']),
    layout: new Set(['Layout']), paint: new Set(['PrePaint', 'Paint', 'Layerize']),
    gc: new Set(mainEvents.filter(e => /^(MajorGC|MinorGC|V8\.GC)/.test(e.name)).map(e => e.name)),
  };
  const maps = new Map((trace.metadata.sourceMaps || []).filter(m => m.sourceMap).map(m => [m.url, new TraceMap(m.sourceMap)]));
  if (n === 3) for (const [url, map] of maps) companionMaps.set(url, map);
  const frameCache = new Map();
  function frame(cf) {
    const key = JSON.stringify(cf);
    if (frameCache.has(key)) return frameCache.get(key);
    let url = cf.url || '', line = (cf.lineNumber ?? -1) + 1, column = cf.columnNumber ?? 0;
    let name = cf.functionName || '(anonymous)';
    const generatedUrl = url;
    const map = maps.get(url) || companionMaps.get(url);
    let mapping = 'unmapped';
    if (map && line > 0 && column >= 0) {
      const original = originalPositionFor(map, { line, column });
      if (original.source) {
        mapping = maps.has(generatedUrl) ? 'trace' : 'companion-3tab-same-url';
        url = original.source; line = original.line; name = original.name || name;
      }
    }
    const result = { name, url, line, mapping, generatedUrl };
    frameCache.set(key, result);
    return result;
  }
  const profiles = events.filter(e => onMain(e) && e.name === 'Profile');
  assert.equal(profiles.length, 1, 'Expected one main-thread CPU profile');
  const self = new Map(), inclusive = new Map(), callers = new Map(), timingCallers = new Map();
  let sampledUs = 0, negativeDeltas = 0, unknownNodes = 0;
  const add = (map, key, dt) => map.set(key, (map.get(key) || 0) + dt);
  for (const profile of profiles) {
    const chunks = events.filter(e => e.name === 'ProfileChunk' && e.pid === profile.pid && e.id === profile.id).sort((a, b) => a.ts - b.ts);
    const nodes = new Map();
    for (const c of chunks) for (const node of c.args.data.cpuProfile?.nodes || []) nodes.set(node.id, node);
    let time = profile.args.data.startTime;
    const points = [];
    for (const c of chunks) {
      const ids = c.args.data.cpuProfile?.samples || [], deltas = c.args.data.timeDeltas || [];
      assert.equal(ids.length, deltas.length, 'Sample/timeDelta length mismatch');
      for (let i = 0; i < ids.length; i++) {
        time += deltas[i];
        if (deltas[i] < 0) negativeDeltas++;
        points.push({ id: ids[i], time });
      }
    }
    const chains = new Map();
    for (const p of sampleSpans(points)) {
      const dt = overlap(p.time, p.end, windows);
      if (!dt) continue;
      sampledUs += dt;
      let node = nodes.get(p.id);
      if (!node) { unknownNodes++; continue; }
      if (!chains.has(p.id)) {
        const seenIds = new Set(), seenFrames = new Set();
        const chain = [];
        while (node && !seenIds.has(node.id)) {
          seenIds.add(node.id);
          const f = frame(node.callFrame), k = JSON.stringify(f);
          seenFrames.add(k);
          chain.push(f);
          node = nodes.get(node.parent);
        }
        const appFrame = chain.find(f => /\/src\/|\/chat-plan-packages\//.test(f.generatedUrl) && !/node_modules/.test(f.generatedUrl));
        const timingParent = ['mark', 'measure'].includes(chain[0].name) && chain.find(f => /@intlify/.test(f.url));
        chains.set(p.id, { leaf: JSON.stringify(chain[0]), frames: [...seenFrames], app: appFrame && JSON.stringify(appFrame),
          timingParent: timingParent && JSON.stringify(timingParent) });
      }
      const chain = chains.get(p.id);
      add(self, chain.leaf, dt);
      for (const k of chain.frames) add(inclusive, k, dt);
      if (chain.app) add(callers, chain.app, dt);
      if (chain.timingParent) add(timingCallers, chain.timingParent, dt);
    }
    // Do not extrapolate the final CPU sample into an unrecorded interval.
    assert(points.at(-1).time > Math.max(...windows.map(w => w[1])), 'CPU profile ends before switch windows');
  }
  assert(sampledUs <= windows.reduce((s, [a, b]) => s + b - a, 0) + 1, 'CPU interval double counting');
  assert.equal(unknownNodes, 0, 'Unknown CPU nodes');
  assert(Math.abs([...self.values()].reduce((s, v) => s + v, 0) - sampledUs) < 1, 'CPU self-time conservation failed');
  const top = (map, limit = 30) => [...map].sort((a, b) => b[1] - a[1]).slice(0, limit).map(([f, us]) => ({ ...JSON.parse(f), ms: round(us / 1000) }));
  const perSwitch = samples.map((s, i) => {
    const ws = [windows[i]];
    const click = clicks.reduce((best, e) => !best || Math.abs(e.ts - ws[0][0]) < Math.abs(best.ts - ws[0][0]) ? e : best, null);
    assert(click && Math.abs(click.ts - ws[0][0]) < 2000, 'Switch not aligned with recorded click');
    return { id: s.id, warm: s.warm, totalMs: round(s.clickToFirstContentMs), activationMs: round(s.routeToActivationMs),
      afterActivationMs: round(s.activationToFirstContentMs), clickOffsetMs: round((click.ts - ws[0][0]) / 1000),
      screenshotFrames: screenshots.filter(e => e.ts >= ws[0][0] && e.ts <= ws[0][1]).length,
      ...Object.fromEntries(Object.entries(categories).map(([k, v]) => [`${k}Ms`, durationFor(v, ws)])) };
  });
  return {
    tabs: n, traceSha256: createHash('sha256').update(raw).digest('hex'), samplesSha256: createHash('sha256').update(samplesRaw).digest('hex'),
    startTime: trace.metadata.startTime, hostDPR: trace.metadata.hostDPR,
    events: events.length, screenshots: screenshots.length,
    screenshotRangePerformanceMs: screenshots.length ? [round((screenshots[0].ts - offset) / 1000), round((screenshots.at(-1).ts - offset) / 1000)] : null,
    mainThread: { pid: main.pid, tid: main.tid }, offsetUs: offset,
    offsetSpreadUs: Math.max(...offsets) - Math.min(...offsets),
    windowTotalMs: round(windows.reduce((sum, [a, b]) => sum + b - a, 0) / 1000),
    durationsMs: Object.fromEntries(Object.entries(categories).map(([k, v]) => [k, durationFor(v)])),
    perSwitch, sampledMs: round(sampledUs / 1000), negativeDeltas, unknownNodes,
    topSelf: top(self), topInclusive: top(inclusive), nearestAppCaller: top(callers),
    intlifyTimingSelfCallers: top(timingCallers),
    relevantInclusive: top(new Map([...inclusive].filter(([k]) => /intlify|ModelSelector|knowledgeBaseSelect|dropdown|scrollbar|recalculateActionCount/.test(k))), 40),
    notes: ['Durations are unions per event category, not additive across categories (JS envelopes can include forced layout).',
      'CPU deltas reconstructed to absolute timestamps, sorted, then weighted by the interval to the next sample. Inclusive stacks overlap and cannot be added; this is not an exact DevTools flamechart reproduction.',
      'Missing 4-tab source maps use companion 3-tab maps only for exactly matching generated URLs; marked on each frame. Such attribution assumes unchanged dev scripts between captures.',
      'Clock anchor: EventTiming.ts - EventTiming.args.data.timeStamp * 1000. Click alignment checked independently.',
      'Only click-to-first-content windows included; idle periods and later console export excluded.'],
  };
}
const results = [analyze(3), analyze(4)];
const output = new URL('docs/analysis/ChatView-R0-trace-analysis-20260922.json', root);
assert(!(process.argv.includes('--write') && process.argv.includes('--check')), 'Choose --write or --check, not both');
const serialized = JSON.stringify(results, null, 2) + '\n';
if (process.argv.includes('--write')) writeFileSync(output, serialized);
if (process.argv.includes('--check')) assert.equal(readFileSync(output, 'utf8'), serialized, 'Derived report is out of date');
console.log(JSON.stringify(results.map(r => ({ tabs: r.tabs, switches: r.perSwitch.length,
  windowTotalMs: r.windowTotalMs, sampledMs: r.sampledMs, durationsMs: r.durationsMs,
  topSelf: r.topSelf.slice(0, 3), nearestAppCaller: r.nearestAppCaller.slice(0, 3) })), null, 2));
