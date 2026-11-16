// Reproducible R0 inventory. Reads source only; --write updates the derived report.
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';
import { parse } from '@vue/compiler-sfc';

const root = new URL('../', import.meta.url);
const sourcePath = 'src/views/ChatView.vue';
const reportPath = 'docs/analysis/ChatView-R0-源码清单-20260922.json';
const source = readFileSync(new URL(sourcePath, root), 'utf8');
const { descriptor, errors } = parse(source, { filename: sourcePath });
if (errors.length) throw new Error(errors.map(String).join('\n'));
const script = descriptor.scriptSetup;
if (!script || descriptor.script) throw new Error('Expected a single script setup block');
const ast = ts.createSourceFile(sourcePath + '.ts', script.content, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
if (ast.parseDiagnostics.length) {
  throw new Error(ast.parseDiagnostics.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
}

const lineOf = node => script.loc.start.line + ast.getLineAndCharacterOfPosition(node.getStart(ast)).line;
const compact = node => node.getText(ast).replace(/\s+/g, ' ').slice(0, 220);
const reactiveNames = new Set(['ref', 'shallowRef', 'reactive', 'shallowReactive', 'computed', 'storeToRefs']);
const effectNames = new Set(['watch', 'watchEffect', 'onMounted', 'onActivated', 'onDeactivated', 'onUnmounted', 'onBeforeUnmount']);
const resourceNames = new Set([
  'listen', 'addEventListener', 'removeEventListener', 'setTimeout', 'clearTimeout',
  'setInterval', 'clearInterval', 'requestAnimationFrame', 'cancelAnimationFrame',
  'requestIdleCallback', 'cancelIdleCallback', 'ResizeObserver', 'MutationObserver',
  'IntersectionObserver', 'createObjectURL', 'revokeObjectURL', 'AbortController',
]);
const imports = [];
const bindings = [];
const effects = [];
const resources = [];
const awaits = [];
const functions = [];

function callName(node) {
  if (!node) return '';
  return ts.isPropertyAccessExpression(node) ? node.name.text : node.getText(ast);
}

function enclosing(node) {
  for (let p = node.parent; p; p = p.parent) {
    if (ts.isFunctionDeclaration(p) && p.name) return p.name.text;
    if (ts.isArrowFunction(p) || ts.isFunctionExpression(p)) {
      if (ts.isVariableDeclaration(p.parent)) return p.parent.name.getText(ast);
      if (ts.isCallExpression(p.parent)) return `${callName(p.parent.expression)} callback@${lineOf(p.parent)}`;
      if (ts.isPropertyAssignment(p.parent)) return `${p.parent.name.getText(ast)} callback@${lineOf(p.parent)}`;
    }
  }
  return 'setup';
}

for (const statement of ast.statements) {
  if (ts.isImportDeclaration(statement)) imports.push({ line: lineOf(statement), module: statement.moduleSpecifier.text });
  if (ts.isVariableStatement(statement)) {
    for (const declaration of statement.declarationList.declarations) {
      const init = declaration.initializer;
      const factory = init && ts.isCallExpression(init) ? callName(init.expression) : null;
      bindings.push({
        line: lineOf(declaration), name: declaration.name.getText(ast), factory,
        reactive: reactiveNames.has(factory), initializer: init ? compact(init) : null,
      });
    }
  }
  if (ts.isFunctionDeclaration(statement)) {
    functions.push({ line: lineOf(statement), name: statement.name?.text,
      async: !!statement.modifiers?.some(m => m.kind === ts.SyntaxKind.AsyncKeyword) });
  }
}

function visit(node) {
  if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
    const name = callName(node.expression);
    const entry = { line: lineOf(node), name, owner: enclosing(node), expression: compact(node) };
    if (effectNames.has(name)) effects.push(entry);
    if (resourceNames.has(name)) resources.push(entry);
  }
  if (ts.isAwaitExpression(node)) awaits.push({ line: lineOf(node), owner: enclosing(node), expression: compact(node.expression) });
  ts.forEachChild(node, visit);
}
visit(ast);

const report = {
  schemaVersion: 1, source: sourcePath,
  sourceSha256: createHash('sha256').update(source).digest('hex'),
  scope: 'ChatView script setup only. Call sites are static, not live resource counts. Semantic ownership is documented in the R0 report.',
  counts: {
    sourceLines: source.split('\n').length, imports: imports.length,
    topLevelBindings: bindings.length, reactiveBindings: bindings.filter(b => b.reactive).length,
    effects: effects.length, resourceOperations: resources.length, awaits: awaits.length,
    topLevelFunctions: functions.length,
  },
  imports, bindings, effects, resources, awaits, functions,
};
const serialized = JSON.stringify(report, null, 2) + '\n';
const mode = process.argv[2];
if (mode === '--write') writeFileSync(new URL(reportPath, root), serialized);
else if (mode === '--check') {
  if (readFileSync(new URL(reportPath, root), 'utf8') !== serialized) throw new Error('Inventory differs from current source; review and regenerate it');
} else if (mode !== undefined) throw new Error('Usage: node scripts/audit-chatview-architecture.mjs [--write|--check]');
console.log(JSON.stringify({ mode: mode ?? 'summary', report: fileURLToPath(new URL(reportPath, root)), ...report.counts }, null, 2));
