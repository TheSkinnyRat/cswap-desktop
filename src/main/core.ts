import { EventEmitter } from 'node:events'
import { existsSync } from 'node:fs'
import { Notification } from 'electron'
import pkg from '../../package.json'
import { CswapDriver, parseMappings, parseUnclaimed } from './cswap/driver'
import { AutoSwitchRunner } from './cswap/auto'
import { findUv, resolveCswap } from './cswap/resolve'
import { SettingsStore } from './settings'
import { VaultWatcher, defaultVaultPath, readMappingsFile, vaultFromSettingsPath } from './vault'
import type {
  AccountsState,
  AutoEvent,
  CswapBinaryInfo,
  Mapping,
  PlainOutput,
  Result,
  UnclaimedEntry
} from '@shared/types'

// Everything the IPC layer needs, in one place. Emits:
//   'accounts' (AccountsState) · 'autoEvent' · 'autoStatus' · 'log' · 'settings'
export class AppCore extends EventEmitter {
  readonly driver = new CswapDriver()
  readonly auto = new AutoSwitchRunner()
  readonly settings: SettingsStore
  readonly vault = new VaultWatcher()
  private binary: CswapBinaryInfo = { path: null, source: 'none', version: null, candidates: [] }
  private accounts: AccountsState = { payload: null, fetchedAt: null, refreshing: false, error: null }
  private refreshTimer: NodeJS.Timeout | null = null
  private inflight: Promise<AccountsState> | null = null
  private vaultPath: string | null = null

  constructor(userData: string) {
    super()
    this.settings = new SettingsStore(userData)
    this.driver.on('log', (e) => this.emit('log', e))
    this.auto.on('event', (ev: AutoEvent) => this.onAutoEvent(ev))
    this.auto.on('status', (st) => this.emit('autoStatus', st))
    this.settings.on('change', (s) => {
      this.emit('settings', s)
      this.scheduleRefresh()
    })
    this.vault.on('change', () => void this.refresh())
  }

  async init(): Promise<void> {
    await this.detectBinary()
    this.scheduleRefresh()
    if (this.binary.path) {
      void this.refresh()
      if (this.settings.get().autoStartAutoSwitch) this.auto.start(this.binary.path, { dryRun: this.settings.get().autoDryRun })
    }
  }

  // ---- binary --------------------------------------------------------------
  getBinaryInfo(): CswapBinaryInfo {
    return this.binary
  }
  async detectBinary(): Promise<CswapBinaryInfo> {
    this.binary = await resolveCswap(this.settings.get().cswapPath)
    this.driver.setBinary(this.binary.version ? this.binary.path : null)
    await this.resolveVault()
    return this.binary
  }
  private async resolveVault(): Promise<void> {
    let vault: string | null = null
    if (this.driver.getBinary()) vault = vaultFromSettingsPath(await this.driver.configPath())
    if (!vault || !existsSync(vault)) vault = existsSync(defaultVaultPath()) ? defaultVaultPath() : vault
    this.vaultPath = vault
    this.vault.watch(vault)
  }
  getVaultPath(): string | null {
    return this.vaultPath
  }
  async installCswap(): Promise<Result<PlainOutput>> {
    const uv = findUv()
    if (!uv) return { ok: false, error: { type: 'NotFound', message: 'uv was not found. Install uv first (https://docs.astral.sh/uv/), then retry.' } }
    const { execFile } = await import('node:child_process')
    return new Promise((resolve) => {
      execFile(uv, ['tool', 'install', 'claude-swap'], { timeout: 300000, windowsHide: true }, async (err, stdout, stderr) => {
        if (err) return resolve({ ok: false, error: { type: 'InstallError', message: (stderr || err.message).trim() } })
        await this.detectBinary()
        void this.refresh()
        resolve({ ok: true, value: { stdout: String(stdout), stderr: String(stderr), exitCode: 0 } })
      })
    })
  }

  // ---- accounts ------------------------------------------------------------
  getAccounts(): AccountsState {
    return this.accounts
  }
  refresh(): Promise<AccountsState> {
    if (this.inflight) return this.inflight
    if (!this.driver.getBinary()) {
      this.accounts = { ...this.accounts, refreshing: false, error: { type: 'NotFound', message: 'cswap is not installed or not found' } }
      this.emit('accounts', this.accounts)
      return Promise.resolve(this.accounts)
    }
    this.accounts = { ...this.accounts, refreshing: true }
    this.emit('accounts', this.accounts)
    this.inflight = (async () => {
      const r = await this.driver.list()
      this.accounts = r.ok
        ? { payload: r.value, fetchedAt: new Date().toISOString(), refreshing: false, error: null }
        : { ...this.accounts, refreshing: false, error: r.error }
      this.inflight = null
      this.emit('accounts', this.accounts)
      return this.accounts
    })()
    return this.inflight
  }
  private scheduleRefresh(): void {
    if (this.refreshTimer) clearInterval(this.refreshTimer)
    const secs = Math.max(15, this.settings.get().refreshSeconds || 60)
    this.refreshTimer = setInterval(() => void this.refresh(), secs * 1000)
  }

  // Every mutation ends with a refresh so the UI never shows a stale row.
  async afterMutation<T>(r: Result<T>): Promise<Result<T>> {
    void this.refresh()
    return r
  }

  // ---- mappings ------------------------------------------------------------
  async listMappings(): Promise<Result<Mapping[]>> {
    const accounts = this.accounts.payload?.accounts ?? []
    const byEmail = (email: string, org?: string) =>
      accounts.find((a) => a.email === email && (!org || a.organizationUuid === org)) ?? accounts.find((a) => a.email === email)
    if (this.vaultPath) {
      const file = readMappingsFile(this.vaultPath)
      if (file) {
        return {
          ok: true,
          value: Object.entries(file)
            .map(([path, v]) => ({ path, email: v.email ?? '', organizationUuid: v.organizationUuid, account: byEmail(v.email ?? '', v.organizationUuid) }))
            .sort((a, b) => a.path.localeCompare(b.path))
        }
      }
    }
    const r = await this.driver.listMappingsText()
    if (!r.ok) return r
    return { ok: true, value: parseMappings(r.value.stdout).map((m) => ({ path: m.path, email: m.email, account: byEmail(m.email) })) }
  }

  async listUnclaimed(): Promise<Result<UnclaimedEntry[]>> {
    const r = await this.driver.unclaimedText()
    if (!r.ok) return r
    return { ok: true, value: parseUnclaimed(r.value.stdout) }
  }

  // ---- auto-switch ---------------------------------------------------------
  private onAutoEvent(ev: AutoEvent): void {
    this.emit('autoEvent', ev)
    if (ev.event === 'switch' || ev.event === 'account-quarantined' || ev.event === 'all-exhausted') void this.refresh()
    if (ev.event === 'switch' && this.settings.get().notifyOnAutoSwitch && Notification.isSupported()) {
      const to = ev.to as { number?: number; email?: string } | null
      const n = new Notification({
        title: ev.dryRun ? 'cswap would switch (dry run)' : 'cswap switched account',
        body: to ? `Now on Account-${to.number ?? '?'} (${to.email ?? ''}) · ${String(ev.trigger ?? '')}` : String(ev.trigger ?? '')
      })
      n.show()
    }
  }

  dispose(): void {
    if (this.refreshTimer) clearInterval(this.refreshTimer)
    this.vault.close()
    void this.auto.stop()
  }
}

export function appInfo(): { version: string; platform: NodeJS.Platform; electron: string } {
  return { version: pkg.version, platform: process.platform, electron: process.versions.electron ?? '' }
}
