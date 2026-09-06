import { afterEach, describe, expect, it } from 'vitest'
import { mkdtempSync, rmSync } from 'node:fs'
import { homedir, tmpdir } from 'node:os'
import { join } from 'node:path'
import { launchSession } from '../../src/main/session'

process.env.CSWAP_DESKTOP_TEST_NO_SPAWN = '1' // never open a terminal from a test run
let dir: string | null = null
afterEach(() => {
  if (dir) rmSync(dir, { recursive: true, force: true })
  dir = null
})

describe('launchSession', () => {
  it('starts in the directory it is given', () => {
    dir = mkdtempSync(join(tmpdir(), 'cswap-run-'))
    const r = launchSession('/usr/bin/cswap', '2', dir)
    expect(r).toEqual({ ok: true, value: { command: 'cswap run 2', cwd: dir } })
  })
  it('falls back to home when no directory is asked for', () => {
    expect(launchSession('/usr/bin/cswap', '2')).toEqual({ ok: true, value: { command: 'cswap run 2', cwd: homedir() } })
    expect(launchSession('/usr/bin/cswap', '2', '   ')).toEqual({ ok: true, value: { command: 'cswap run 2', cwd: homedir() } })
  })
  it('refuses a directory that is not there instead of opening a terminal somewhere else', () => {
    const r = launchSession('/usr/bin/cswap', '2', join(tmpdir(), 'cswap-run-definitely-missing'))
    expect(r.ok).toBe(false)
    expect(!r.ok && r.error.message).toMatch(/No such directory/)
  })
  it('needs a binary', () => {
    const r = launchSession(null, '2')
    expect(r.ok).toBe(false)
    expect(!r.ok && r.error.type).toBe('NotFound')
  })
})
