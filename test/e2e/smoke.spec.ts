import { test, expect } from '@playwright/test'
import { launch, shot } from './helpers'

test('renders every page in light and dark and lists the fake accounts', async () => {
  const { app, page } = await launch()
  await expect(page.getByTestId('accounts-table')).toBeVisible({ timeout: 20000 })
  await expect(page.getByTestId('account-row-1')).toHaveAttribute('data-active', 'true')
  await expect(page.getByTestId('sidebar-status')).toContainText('cswap 0.25.0')
  await shot(page, 'accounts-light')
  // eye button masks emails everywhere (rows, subtitle, title-bar pill) and persists as a setting
  await page.getByTestId('mask-toggle').click()
  await expect(page.getByTestId('account-row-2')).toContainText('wor•••')
  await expect(page.getByTestId('account-row-2')).not.toContainText('work@company.com')
  await expect(page.getByTestId('account-row-2')).not.toContainText('Company Inc') // the org names an employer as plainly as the address does
  await expect(page.getByTestId('active-pill')).toContainText('main')
  await shot(page, 'accounts-masked')
  await page.getByTestId('mask-toggle').click()
  await expect(page.getByTestId('account-row-2')).toContainText('work@company.com')
  // the subscription is read from the live credential file and labelled on the active row
  await expect(page.getByTestId('account-row-1')).toContainText('Max 5x')
  await expect(page.getByTestId('account-row-2')).not.toContainText('Max 5x')
  // per-model window sits under the 7-day column
  await expect(page.getByTestId('account-row-1').getByTestId('scoped-Fable')).toContainText('Fable 17%')
  await expect(page.getByTestId('account-row-1')).not.toContainText('left') // a duration, not a sentence
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
