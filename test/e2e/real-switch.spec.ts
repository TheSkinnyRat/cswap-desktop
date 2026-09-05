import { test, expect } from '@playwright/test'
import { execFileSync } from 'node:child_process'
import { launch, shot } from './helpers'

// ONE real switch and back, through the UI, against the installed cswap.
// Opt in with CSWAP_REAL_SWITCH=<slot to switch to>. Nothing is added or removed.
const target = process.env.CSWAP_REAL_SWITCH
test.skip(!target, 'set CSWAP_REAL_SWITCH=<slot> to run a real switch + switch back')

function activeSlot(): number | null {
  const out = execFileSync('cswap', ['status', '--json'], { encoding: 'utf8', shell: process.platform === 'win32' })
  return (JSON.parse(out).active?.number as number | undefined) ?? null
}

test('switches to the target slot and back, and cswap status agrees each time', async () => {
  const origin = activeSlot()
  expect(origin).not.toBeNull()
  expect(String(origin)).not.toBe(target)
  const { app, page } = await launch({ bin: '' })
  await expect(page.getByTestId('accounts-table')).toBeVisible({ timeout: 60000 })
  await expect(page.getByTestId(`account-row-${origin}`)).toHaveAttribute('data-active', 'true')

  await page.getByTestId(`switch-${target}`).click()
  await expect(page.getByRole('status')).toContainText(`Switched to Account-${target}`, { timeout: 60000 })
  await expect(page.getByTestId(`account-row-${target}`)).toHaveAttribute('data-active', 'true', { timeout: 60000 })
  expect(activeSlot()).toBe(Number(target))
  await shot(page, 'real-switched')

  await page.getByTestId(`switch-${origin}`).click()
  await expect(page.getByRole('status').last()).toContainText(`Switched to Account-${origin}`, { timeout: 60000 })
  await expect(page.getByTestId(`account-row-${origin}`)).toHaveAttribute('data-active', 'true', { timeout: 60000 })
  expect(activeSlot()).toBe(origin)
  await shot(page, 'real-switched-back')
  await app.close()
})
