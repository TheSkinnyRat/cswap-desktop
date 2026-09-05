import { test, expect } from '@playwright/test'
import { launch, shot } from './helpers'

// Read-only check against the REAL cswap on this machine. Opt in with CSWAP_REAL=1.
// It never switches, adds or removes anything — it only proves detection + `list --json`.
test.skip(!process.env.CSWAP_REAL, 'set CSWAP_REAL=1 to run against the installed cswap')

test('detects the installed cswap and lists real accounts without mutating anything', async () => {
  const { app, page } = await launch({ bin: '', env: { CSWAP_DESKTOP_BIN: '' } })
  await expect(page.getByTestId('sidebar-status')).toContainText(/cswap \d+\.\d+/, { timeout: 30000 })
  await expect(page.getByTestId('accounts-table')).toBeVisible({ timeout: 60000 })
  const rows = page.locator('[data-testid^="account-row-"]')
  expect(await rows.count()).toBeGreaterThan(0)
  await expect(page.locator('[data-active="true"]')).toHaveCount(1)
  await shot(page, 'real-accounts')
  await page.getByTestId('nav-log').click()
  const cmds = await page.locator('[data-testid="log-row"] .mono').allTextContents()
  // every call so far must be a read
  for (const c of cmds) expect(c).toMatch(/^cswap (list --json|config path|map|status --json|--version)$/)
  await page.getByTestId('nav-settings').click()
  await shot(page, 'real-settings')
  await app.close()
})
