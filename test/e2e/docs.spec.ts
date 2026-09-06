import { test } from '@playwright/test'
import { chmodSync, copyFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { launch, shot, ROOT } from './helpers'

// Regenerates the README images. Opt in with DOCS=1 — it is a rendering job, not a check.
// The fake cswap is copied to a neutral path first: whatever the app prints in Settings
// ends up in a public repository, and a developer's own directory layout should not.
test.skip(!process.env.DOCS, 'set DOCS=1 to regenerate the README screenshots')

const seed = {
  version: '0.25.0',
  activeAccountNumber: 1,
  accounts: {
    1: { email: 'you@example.com', organizationName: '', organizationUuid: '', alias: 'main', usage: { five_hour: 45, seven_day: 12, scoped: [{ name: 'Fable', pct: 17 }] } },
    2: { email: 'work@example.com', organizationName: 'Acme Inc', organizationUuid: 'org-2', usage: { five_hour: 100, seven_day: 61, scoped: [{ name: 'Fable', pct: 100 }] }, resetsIn5h: 1800 },
    3: { email: 'spare@example.com', organizationName: '', organizationUuid: '', disabled: true, usage: { five_hour: 3, seven_day: 40 } },
    4: { email: 'api-key-4@token.local', organizationName: '', organizationUuid: '', usageStatus: 'api_key' }
  },
  sequence: [1, 2, 3, 4],
  mappings: { '/home/you/work/client-app': { email: 'work@example.com', organizationUuid: 'org-2' } },
  settings: {}, unclaimed: {}, liveIdentity: 'you@example.com', log: []
}

test('README screenshots', async () => {
  const neutral = join(tmpdir(), 'claude-swap-docs', 'bin')
  mkdirSync(neutral, { recursive: true })
  const bin = join(neutral, process.platform === 'win32' ? 'cswap.cmd' : 'cswap')
  copyFileSync(join(ROOT, 'test', 'fake-cswap', 'fake-cswap.mjs'), join(neutral, 'fake-cswap.mjs'))
  writeFileSync(bin, process.platform === 'win32' ? '@echo off\r\nnode "%~dp0fake-cswap.mjs" %*\r\n' : `#!/bin/sh\nexec node "${join(neutral, 'fake-cswap.mjs')}" "$@"\n`)
  chmodSync(bin, 0o755)

  const { app, page } = await launch({ seedState: seed, bin })
  await page.setViewportSize({ width: 1240, height: 780 })
  await page.waitForSelector('[data-testid="accounts-table"]')
  await page.waitForTimeout(500)
  await shot(page, 'doc-accounts-light')

  await page.keyboard.press('Control+k')
  await page.waitForTimeout(250)
  await shot(page, 'doc-palette')
  await page.keyboard.press('Escape')

  await page.getByTestId('nav-auto').click()
  await page.getByTestId('auto-toggle').click()
  await page.waitForTimeout(1200)
  await page.getByRole('button', { name: 'All' }).click()
  await shot(page, 'doc-auto')
  await page.getByTestId('auto-toggle').click()
  await page.waitForTimeout(5200) // let the toasts clear before the next frame

  await page.getByTestId('nav-settings').click()
  await page.waitForTimeout(600)
  await shot(page, 'doc-settings-light')

  await page.getByRole('radio', { name: 'Dark' }).click()
  await page.getByTestId('nav-accounts').click()
  await page.waitForTimeout(400)
  await shot(page, 'doc-accounts-dark')
  await app.close()
})
