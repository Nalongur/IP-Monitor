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
  let expansions = 0
  const monitor = scope.run(() => useIpMonitor((command, payload) => {
    if (command === 'set_detection_mode') return Promise.resolve()
    if (command === 'expand_edge') {
      assert.equal(payload.automatic, true)
      expansions += 1
      return Promise.resolve()
    }
    assert.equal(command, 'get_ip_info')
    return new Promise((resolve, reject) => requests.push({ mode: payload.mode, resolve, reject }))
  }))
  const direct = 'direct'
  const system = 'system_proxy'
  const custom = 'custom_proxy'
  const modes = [direct, system, custom]
  const base = { [direct]: '192.0.2.42', [system]: '198.51.100.42', [custom]: null }
  const tick = () => [...timers.values()].forEach(callback => callback())

  async function complete(batch, addresses, ipv6) {
    for (const request of batch) {
      if (request.mode === custom && !addresses[custom]) {
        request.reject(new Error('No custom proxy configured'))
        continue
      }
      const ip = addresses[request.mode]
      request.resolve({
        mode: request.mode,
        ipv4Status: ip ? 'success' : 'unavailable',
        ipv4: ip ? { ip } : null,
        ipv6Status: request.mode === system && ipv6 ? 'success' : 'unavailable',
        ipv6: request.mode === system ? ipv6 : null,
      })
    }
    await new Promise(resolve => setImmediate(resolve))
  }

  async function completeTick(addresses, ipv6) {
    const before = requests.length
    tick()
    const batch = requests.slice(before)
    assert.deepEqual(batch.map(request => request.mode).sort(), [...modes].sort())
    tick()
    assert.equal(requests.length, before + modes.length, 'skip while the previous tick is pending')
    await complete(batch, addresses, ipv6)
  }

  monitor.startAutoRefresh()
  monitor.startAutoRefresh()
  assert.equal(timers.size, 1)
  assert.equal(requests.length, 3, 'all modes establish a baseline on startup')
  tick()
  assert.equal(requests.length, 3, 'skip while startup detection is pending')
  await complete(requests.slice(), base)
  assert.equal(expansions, 0, 'first observations must not expand')

  const changed = { ...base, [system]: '198.51.100.43' }
  await completeTick(changed)
  assert.equal(expansions, 1, 'system proxy IP change expands while Direct is selected')
  assert.equal(monitor.result.value.mode, direct)
  assert.equal(monitor.result.value.ipv4.ip, base[direct])
  await completeTick(changed)
  assert.equal(expansions, 1, 'same addresses do not expand again')
  await completeTick({ ...changed, [system]: null })
  await completeTick(changed, '2001:db8::42')
  assert.equal(expansions, 1, 'temporary failure and first IPv6 do not expand')
  await completeTick(changed, '2001:db8::43')
  assert.equal(expansions, 2, 'background IPv6 change expands')

  const configured = { ...changed, [custom]: '203.0.113.42' }
  await completeTick(configured, '2001:db8::43')
  assert.equal(expansions, 2, 'first custom proxy address establishes a baseline')
  await completeTick({ ...configured, [custom]: '203.0.113.43' }, '2001:db8::43')
  assert.equal(expansions, 3, 'background custom proxy change expands')

  const count = requests.length
  scope.stop()
  assert.equal(timers.size, 0, 'dispose clears the timer')
  tick()
  assert.equal(requests.length, count)
  console.log('120-second cadence, IP change detection, in-flight skips and disposal passed')
} finally {
  scope.stop()
  globalThis.setInterval = originalSet
  globalThis.clearInterval = originalClear
  await server.close()
}
