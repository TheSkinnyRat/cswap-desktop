import { test, expect } from '@playwright/test'
import { launch, shot } from './helpers'

// Opens a real session-mode terminal (`cswap run <slot>`) from the row menu.
// Opt in with CSWAP_REAL_RUN=<slot>. The terminal (and the Claude Code it starts)
// is left for the caller to close — this only proves the spawn path on this OS.
const slot = process.env.CSWAP_REAL_RUN
test.skip(!slot, 'set CSWAP_REAL_RUN=<slot> to open a real session terminal')

test('row menu opens a terminal running cswap run <slot>', async () => {
  const { app, page } = await launch({ bin: '' })
  await expect(page.getByTestId('accounts-table')).toBeVisible({ timeout: 60000 })
  await page.getByTestId(`row-menu-${slot}`).click()
  await page.getByRole('menuitem', { name: /Open terminal as this account/ }).click()
  await expect(page.getByRole('status')).toContainText(`cswap run ${slot}`, { timeout: 15000 })
  await shot(page, 'real-run')
  // With a detached console child alive, Playwright's close() has been seen to hang on
  // Windows even though the app itself exits fine — do not let that fail the proof.
  await Promise.race([app.close(), new Promise((r) => setTimeout(r, 10000))])
  if (app.process().exitCode === null) app.process().kill()
})
