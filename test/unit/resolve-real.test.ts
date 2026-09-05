import { describe, expect, it } from 'vitest'
import { candidatePaths, resolveCswap } from '../../src/main/cswap/resolve'

// Opt-in: proves detection against the cswap installed on this machine.
describe.skipIf(!process.env.CSWAP_REAL)('resolveCswap (real machine)', () => {
  it('finds the installed cswap without any override', async () => {
    delete process.env.CSWAP_DESKTOP_BIN
    const info = await resolveCswap(null)
    console.log(JSON.stringify(info, null, 2), candidatePaths())
    expect(info.version).toMatch(/^\d+\.\d+\.\d+/)
    expect(info.path).toBeTruthy()
  })
})
