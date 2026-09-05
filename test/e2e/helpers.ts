import { _electron as electron, type ElectronApplication, type Page } from '@playwright/test'
import { mkdtempSync, writeFileSync, existsSync, readFileSync, mkdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const ROOT = resolve(fileURLToPath(new URL('.', import.meta.url)), '..', '..')
export const FAKE = process.platform === 'win32' ? join(ROOT, 'test', 'fake-cswap', 'fake-cswap.cmd') : join(ROOT, 'test', 'fake-cswap', 'fake-cswap.mjs')
export const SHOTS = join(ROOT, 'test-results', 'shots')

export interface FakeState {
  activeAccountNumber: number
  accounts: Record<string, Record<string, unknown>>
  mappings: Record<string, { email: string; organizationUuid?: string }>
  settings: Record<string, unknown>
  log: string[][]
}

export interface Launched {
  app: ElectronApplication
  page: Page
  statePath: string
  userData: string
  state(): FakeState
}

// Launches the built app against the fake cswap with a fresh vault + userData.
export async function launch(opts: { seedState?: object; settings?: object; env?: Record<string, string>; bin?: string } = {}): Promise<Launched> {
  const dir = mkdtempSync(join(tmpdir(), 'cswap-desktop-e2e-'))
  const statePath = join(dir, 'vault', 'state.json')
  mkdirSync(join(dir, 'vault'), { recursive: true })
  if (opts.seedState) writeFileSync(statePath, JSON.stringify(opts.seedState, null, 2))
  const userData = join(dir, 'userData')
  mkdirSync(userData, { recursive: true })
  if (opts.settings) writeFileSync(join(userData, 'settings.json'), JSON.stringify(opts.settings))
  mkdirSync(SHOTS, { recursive: true })
  const env: Record<string, string> = {
    ...(process.env as Record<string, string>),
    CSWAP_DESKTOP_USERDATA: userData,
    FAKE_CSWAP_STATE: statePath,
    ELECTRON_ENABLE_LOGGING: '0',
    ...opts.env
  }
  if (opts.bin === '') delete env.CSWAP_DESKTOP_BIN
  else env.CSWAP_DESKTOP_BIN = opts.bin ?? FAKE
  const app = await electron.launch({
    args: [join(ROOT, 'out', 'main', 'index.js'), '--no-sandbox'],
    env
  })
  const page = await app.firstWindow()
  await page.waitForLoadState('domcontentloaded')
  await page.setViewportSize({ width: 1120, height: 720 })
  return {
    app,
    page,
    statePath,
    userData,
    state: (): FakeState => (existsSync(statePath) ? (JSON.parse(readFileSync(statePath, 'utf8')) as FakeState) : { activeAccountNumber: 0, accounts: {}, mappings: {}, settings: {}, log: [] })
  }
}

export async function shot(page: Page, name: string): Promise<void> {
  await page.waitForTimeout(250)
  await page.screenshot({ path: join(SHOTS, `${name}.png`), animations: 'disabled' })
}
