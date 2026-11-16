import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const packages = [
  ['tauri-download-center-core', 'download-packages/packages/tauri-download-center-core/package.json'],
  ['tauri-download-center-vue', 'download-packages/packages/tauri-download-center-vue/package.json'],
]

for (const [expectedName, manifestPath] of packages) {
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
  assert.equal(manifest.name, expectedName)
  assert.ok(manifest.version)
}

console.log('download center package manifests verified')
