import { test, expect } from '@playwright/test'
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
  for (const width of [900, 1024, 1327, 1600, 1920]) {
    await page.setViewportSize({ width, height: 620 })
    await page.waitForTimeout(150)
    const cols = await page.evaluate(() => {
      const table = document.querySelector('[data-testid="accounts-table"]')!
      return Array.from(table.children).map((row) =>
        Array.from(row.children).map((c) => Math.round((c as HTMLElement).getBoundingClientRect().left))
      )
    })
    const [header, ...rows] = cols
    for (const [i, row] of rows.entries()) expect(row, `width ${width}, row ${i + 1}`).toEqual(header)
  }
  await page.setViewportSize({ width: 1327, height: 620 })
  await shot(page, 'align-wide')
  // a narrow window scrolls the table instead of squeezing it
  await page.setViewportSize({ width: 760, height: 620 })
  await page.waitForTimeout(150)
  const scroll = await page.evaluate(() => {
    const el = document.querySelector('[data-testid="accounts-scroller"]') as HTMLElement
    el.scrollLeft = 9999
    return { canScroll: el.scrollWidth > el.clientWidth + 4, scrolled: el.scrollLeft > 0 }
  })
  expect(scroll.canScroll).toBe(true)
  expect(scroll.scrolled).toBe(true)
  await shot(page, 'align-narrow-scrolled')
  await app.close()
})
