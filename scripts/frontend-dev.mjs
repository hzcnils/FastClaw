import http from 'node:http'
import { spawn } from 'node:child_process'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const viteUrl = 'http://127.0.0.1:1420/'

function isViteAlive() {
  return new Promise((resolveProbe) => {
    const request = http.get(viteUrl, (response) => {
      response.resume()
      resolveProbe(response.statusCode === 200)
    })
    request.setTimeout(3000, () => request.destroy())
    request.once('error', () => resolveProbe(false))
  })
}

function waitUntilStopped() {
  const keepAlive = setInterval(() => {}, 60 * 60 * 1000)
  const stop = () => {
    clearInterval(keepAlive)
    process.exit(0)
  }
  process.once('SIGINT', stop)
  process.once('SIGTERM', stop)
}

if (await isViteAlive()) {
  console.log(`[frontend-dev] Reusing Vite on ${viteUrl}`)
  waitUntilStopped()
} else {
  console.log('[frontend-dev] Starting Vite...')
  const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm'
  const vite = spawn(npmCommand, ['run', 'dev'], {
    cwd: root,
    env: process.env,
    shell: process.platform === 'win32',
    stdio: 'inherit',
  })

  let stopping = false
  const forwardSignal = (signal) => {
    stopping = true
    if (vite.exitCode === null) vite.kill(signal)
  }
  process.once('SIGINT', () => forwardSignal('SIGINT'))
  process.once('SIGTERM', () => forwardSignal('SIGTERM'))

  vite.once('error', (error) => {
    console.error('[frontend-dev] Failed to start Vite:', error)
    process.exitCode = 1
  })
  vite.once('exit', (code, signal) => {
    if (stopping) {
      process.exitCode = 0
      return
    }
    if (signal) {
      console.error(`[frontend-dev] Vite stopped by ${signal}`)
      process.exitCode = 1
      return
    }
    process.exitCode = code ?? 1
  })
}
