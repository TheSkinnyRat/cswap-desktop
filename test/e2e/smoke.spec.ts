import { test, expect } from '@playwright/test'
import { launch, shot } from './helpers'

test('renders every page in light and dark and lists the fake accounts', async () => {
  const { app, page } = await launch()
  await expect(page.getByTestId('accounts-table')).toBeVisible({ timeout: 20000 })
  await expect(page.getByTestId('account-row-1')).toHaveAttribute('data-active', 'true')
  await expect(page.getByTestId('sidebar-status')).toContainText('cswap 0.25.0')
  await shot(page, 'accounts-light')
  for (const p of ['auto', 'mappings', 'log', 'settings'] as const) {
    await page.getByTestId(`nav-${p}`).click()
    await expect(page.getByTestId(`page-${p}`)).toBeVisible()
    await shot(page, `${p}-light`)
  }
  // dark
  await page.getByRole('radio', { name: 'Dark' }).click()
  await expect(page.locator('html')).toHaveClass(/dark/)
  await shot(page, 'settings-dark')
  await page.getByTestId('nav-accounts').click()
  await shot(page, 'accounts-dark')
  await page.getByTestId('nav-auto').click()
  await shot(page, 'auto-dark')
  await app.close()
})
