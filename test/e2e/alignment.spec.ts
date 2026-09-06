import { test, expect } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { launch, shot } from './helpers'

// Mixed status labels are the case that broke alignment: every row is its own grid,
// so a content-sized column resolved to a different width per row.
const seed = {
  version: '0.25.0',
  activeAccountNumber: 4,
  accounts: {
    1: { email: 'one@example.com', organizationName: '', organizationUuid: '', usage: { five_hour: 0, seven_day: 8 } },
    2: { email: 'two@example.com', organizationName: '', organizationUuid: '', usageStatus: 'relogin_required' },
    3: { email: 'three@example.com', organizationName: '', organizationUuid: '', usage: { five_hour: 41, seven_day: 21, scoped: [{ name: 'Fable', pct: 3 }] } },
    4: { email: 'four@example.com', organizationName: 'Acme Inc', organizationUuid: 'org-g', usageStatus: 'token_expired' }
  },
  sequence: [1, 2, 3, 4],
  mappings: {}, settings: {}, unclaimed: {}, liveIdentity: 'four@example.com', log: []
}

test('columns line up at every width, whatever the status labels say', async () => {
  const { app, page } = await launch({ seedState: seed })
  await page.waitForSelector('[data-testid="accounts-table"]')
  // Both views: each has its own column template, and each is a grid per row.
  for (const view of ['bars', 'rings'] as const) {
    if (view === 'rings') await page.getByTestId('view-toggle').click()
    await checkWidths(page, view)
  }
  await page.getByTestId('view-toggle').click()
  await page.setViewportSize({ width: 1327, height: 620 })

  // Every bar fills its own cell. The tooltip wrapper is inline-flex, and inside a cell
  // that is not itself the grid item it shrank to its content, taking the track with it —
  // so compare each track against the cell around it, not against the other columns
  // (5-hour and 7-day are deliberately different widths).
  for (const cell of ['window-5h', 'window-7d', 'window-model-Fable']) {
    const outer = await page.getByTestId('account-row-3').getByTestId(cell).boundingBox()
    const track = await page.getByTestId('account-row-3').getByTestId(cell).locator('[role="progressbar"]').first().boundingBox()
    expect(Math.abs(outer!.width - track!.width), `${cell}: cell ${Math.round(outer!.width)}px vs track ${Math.round(track!.width)}px`).toBeLessThan(2)
  }

  await shot(page, 'align-wide')
  await app.close()
})

test('a per-model window sits beside the 7-day one when there is room, under it when there is not', async () => {
  const { app, page } = await launch({ seedState: seed })
  await page.waitForSelector('[data-testid="accounts-table"]')
  const box = async (cell: string) => (await page.getByTestId('account-row-3').getByTestId(cell).boundingBox())!

  await page.setViewportSize({ width: 1700, height: 620 })
  await page.waitForTimeout(250)
  let seven = await box('window-7d')
  let fable = await box('scoped-Fable')
  expect(fable.x, 'wide: beside').toBeGreaterThan(seven.x + seven.width - 1)
  expect(Math.abs(fable.y - seven.y), 'wide: same line').toBeLessThan(6)

  await page.setViewportSize({ width: 1000, height: 620 })
  await page.waitForTimeout(250)
  seven = await box('window-7d')
  fable = await box('scoped-Fable')
  expect(fable.y, 'narrow: below').toBeGreaterThan(seven.y + seven.height - 1)

  // Rings keep a fixed pitch, so a row whose window has no reset line does not shift
  // the per-model ring left of the row above it.
  await page.setViewportSize({ width: 1700, height: 620 })
  await page.getByTestId('view-toggle').click()
  await page.waitForTimeout(300)
  const ringX = async (row: number): Promise<number> => (await page.getByTestId(`account-row-${row}`).getByTestId('window-7d').boundingBox())!.x
  expect(Math.abs((await ringX(3)) - (await ringX(1))), 'rings share a column').toBeLessThan(2)
  await app.close()
})

async function checkWidths(page: import('@playwright/test').Page, view: string): Promise<void> {
  // below ~1040 the list is narrower than a table needs and becomes cards instead
  for (const width of [1100, 1327, 1600, 1920]) {
    await page.setViewportSize({ width, height: 620 })
    await page.waitForTimeout(150)
    const cols = await page.evaluate(() => {
      const table = document.querySelector('[data-testid="accounts-table"]')!
      return Array.from(table.children).map((row) =>
        Array.from(row.children).map((c) => Math.round((c as HTMLElement).getBoundingClientRect().left))
      )
    })
    const [header, ...rows] = cols
    for (const [i, row] of rows.entries()) expect(row, `${view} at ${width}px, row ${i + 1}`).toEqual(header)
  }
}


test('a narrow window changes the shape instead of scrolling sideways', async () => {
  const { app, page, userData } = await launch({ seedState: seed })
  await page.waitForSelector('[data-testid="accounts-table"]')

  await page.setViewportSize({ width: 1240, height: 720 })
  await page.waitForTimeout(300)
  await expect(page.getByTestId('accounts-table')).not.toHaveAttribute('data-compact', 'true')

  for (const width of [560, 700, 900]) {
    await page.setViewportSize({ width, height: 720 })
    await page.waitForTimeout(300)
    await expect(page.getByTestId('accounts-table'), `${width}px: cards`).toHaveAttribute('data-compact', 'true')
    // nothing scrolls sideways: not the page, not the list
    const over = await page.evaluate(() => {
      const el = document.querySelector('[data-testid="accounts-scroller"]') as HTMLElement
      const main = document.querySelector('main') as HTMLElement
      return {
        list: el.scrollWidth - el.clientWidth,
        main: main.scrollWidth - main.clientWidth,
        body: document.documentElement.scrollWidth - document.documentElement.clientWidth
      }
    })
    expect(over, `${width}px: ${JSON.stringify(over)}`).toEqual({ list: 0, main: 0, body: 0 })
    // every window still reads, with its own name now that the header is gone
    await expect(page.getByTestId('account-row-3').getByTestId('window-7d')).toContainText('7-day')
  }
  await shot(page, 'compact-cards')

  // the sidebar collapses on its own when narrow, and by hand when it is not
  await page.setViewportSize({ width: 560, height: 720 })
  await page.waitForTimeout(300)
  await expect(page.locator('nav[aria-label="Main"]')).toHaveAttribute('data-compact', 'true')
  await expect(page.getByTestId('sidebar-toggle')).toHaveCount(0) // forced: nothing to choose
  await page.setViewportSize({ width: 1240, height: 720 })
  await page.waitForTimeout(300)
  await expect(page.locator('nav[aria-label="Main"]')).not.toHaveAttribute('data-compact', 'true')
  await page.getByTestId('sidebar-toggle').click()
  await expect(page.locator('nav[aria-label="Main"]')).toHaveAttribute('data-compact', 'true')
  await expect(page.getByTestId('accounts-table')).not.toHaveAttribute('data-compact', 'true') // more room, still a table
  await app.close()
  expect(JSON.parse(readFileSync(join(userData, 'settings.json'), 'utf8')).sidebarCollapsed).toBe(true)
})


test('a collapsed sidebar explains itself beside the icon, not above it', async () => {
  const { app, page } = await launch({ seedState: seed })
  await page.setViewportSize({ width: 620, height: 720 })
  await page.waitForSelector('[data-testid="accounts-table"]')
  await page.waitForTimeout(300)
  const nav = (await page.locator('nav[aria-label="Main"]').boundingBox())!
  for (const item of ['nav-accounts', 'nav-settings']) {
    await page.getByTestId(item).hover()
    const tip = (await page.getByRole('tooltip').boundingBox())!
    expect(tip.x, `${item}: beside the sidebar`).toBeGreaterThanOrEqual(nav.x + nav.width)
    // the top item's card used to open upward and get cut off by the window
    expect(tip.y, `${item}: inside the window`).toBeGreaterThanOrEqual(0)
    expect(tip.y + tip.height).toBeLessThanOrEqual(720)
  }
  await shot(page, 'sidebar-tooltip')
  await app.close()
})
