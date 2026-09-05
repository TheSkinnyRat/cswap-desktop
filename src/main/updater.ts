import { app } from 'electron'
import { EventEmitter } from 'node:events'
import pkg from '../../package.json'
import type { UpdaterState } from '@shared/types'

const RELEASES_URL = 'https://github.com/TheSkinnyRat/cswap-desktop/releases/latest'

// Auto-update via electron-updater against GitHub releases (latest.yml is published
// by electron-builder). Downloads in the background and installs on quit or on request.
// Only active in the packaged app; dev/e2e get a 'disabled' state and nothing else.
export class Updater extends EventEmitter {
  private state: UpdaterState = { status: app.isPackaged ? 'idle' : 'disabled', current: pkg.version, releaseUrl: RELEASES_URL }
  private timer: NodeJS.Timeout | null = null
  private ready = false
  private auto: typeof import('electron-updater').autoUpdater | null = null

  getState(): UpdaterState {
    return { ...this.state }
  }

  private set(patch: Partial<UpdaterState>): void {
    this.state = { ...this.state, ...patch }
    this.emit('state', this.getState())
  }

  private async init(): Promise<boolean> {
    if (this.ready) return !!this.auto
    this.ready = true
    if (!app.isPackaged || process.platform === 'darwin') {
      // macOS needs a signed app for Squirrel.Mac; unsigned builds stay on the manual path.
      this.set({ status: 'disabled' })
      return false
    }
    const { autoUpdater } = await import('electron-updater')
    this.auto = autoUpdater
    autoUpdater.autoDownload = true
    autoUpdater.autoInstallOnAppQuit = true
    autoUpdater.allowPrerelease = false
    autoUpdater.logger = null
    // Builds are unsigned, so the NSIS signature check has nothing to compare against.
    ;(autoUpdater as unknown as { verifyUpdateCodeSignature: () => Promise<null> }).verifyUpdateCodeSignature = async () => null
    autoUpdater.on('checking-for-update', () => this.set({ status: 'checking', error: undefined }))
    autoUpdater.on('update-available', (info) => this.set({ status: 'available', version: info.version, percent: 0, checkedAt: new Date().toISOString() }))
    autoUpdater.on('update-not-available', (info) => this.set({ status: 'not-available', version: info.version, checkedAt: new Date().toISOString() }))
    autoUpdater.on('download-progress', (p) => this.set({ status: 'downloading', percent: Math.round(p.percent) }))
    autoUpdater.on('update-downloaded', (info) => this.set({ status: 'downloaded', version: info.version, percent: 100 }))
    autoUpdater.on('error', (err) => this.set({ status: 'error', error: err?.message ?? String(err) }))
    return true
  }

  async check(): Promise<UpdaterState> {
    if (!(await this.init()) || !this.auto) return this.getState()
    if (this.state.status === 'downloading' || this.state.status === 'downloaded') return this.getState()
    try {
      await this.auto.checkForUpdates()
    } catch (e) {
      this.set({ status: 'error', error: (e as Error).message })
    }
    return this.getState()
  }

  // Check shortly after start, then every six hours while the app lives.
  schedule(enabled: boolean): void {
    if (this.timer) clearInterval(this.timer)
    this.timer = null
    if (!enabled) return
    setTimeout(() => void this.check(), 15000)
    this.timer = setInterval(() => void this.check(), 6 * 3600 * 1000)
  }

  install(): void {
    if (this.auto && this.state.status === 'downloaded') this.auto.quitAndInstall(false, true)
  }

  dispose(): void {
    if (this.timer) clearInterval(this.timer)
  }
}
