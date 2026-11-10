import assert from 'node:assert/strict'
import { effectScope } from 'vue'
import { createServer } from 'vite'

const server = await createServer({
  server: { middlewareMode: true, hmr: false, watch: null },
  appType: 'custom',
})
const originalSet = globalThis.setInterval
const originalClear = globalThis.clearInterval
const scope = effectScope()
try {
  const { useIpMonitor } = await server.ssrLoadModule('/src/composables/useIpMonitor.ts')
  const timers = new Map()
  let id = 0
  globalThis.setInterval = (callback, milliseconds) => {
    assert.equal(milliseconds, 120_000)
    timers.set(++id, callback)
    return id
  }
  globalThis.clearInterval = (timer) => timers.delete(timer)
  const requests = []
  const monitor = scope.run(() => useIpMonitor((command, payload) => {
    if (command === 'set_detection_mode') return Promise.resolve()
    assert.equal(command, 'get_ip_info')
    return new Promise(resolve => requests.push({ mode: payload.mode, resolve }))
  }))
  const sample = {
    ipv4Status: 'success',
    ipv6Status: 'unavailable',
    ipv4: { ip: '192.0.2.42' },
  }
  monitor.startAutoRefresh()
  monitor.startAutoRefresh()
  assert.equal(timers.size, 1)
  const tick = () => [...timers.values()].forEach(callback => callback())

  const startup = monitor.refresh()
  tick()
  assert.equal(requests.length, 1, 'skip while startup detection is pending')
  const selected = monitor.selectMode('custom_proxy')
  await Promise.resolve()
  assert.equal(requests[1].mode, 'custom_proxy')
  requests[1].resolve(sample)
  await selected
  tick()
  assert.equal(requests.length, 2, 'skip while a superseded request is still in flight')
  requests[0].resolve(sample)
  await startup

  tick()
  assert.equal(requests[2].mode, 'custom_proxy', 'tick uses the selected mode')
  tick()
  assert.equal(requests.length, 3, 'skip while a scheduled request is pending')
  requests[2].resolve(sample)
  await Promise.resolve()
  scope.stop()
  assert.equal(timers.size, 0, 'dispose clears the timer')
  tick()
  assert.equal(requests.length, 3)
  console.log('120-second cadence, mode selection, in-flight skips and disposal passed')
} finally {
  scope.stop()
  globalThis.setInterval = originalSet
  globalThis.clearInterval = originalClear
  await server.close()
}
