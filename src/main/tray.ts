import { Menu, Tray, nativeImage, type BrowserWindow } from 'electron'
import { join } from 'node:path'
import type { AppCore } from './core'
import type { Account } from '@shared/types'

function label(a: Account): string {
  const who = a.alias ? `${a.alias} · ${a.email}` : a.email
  const u = a.usage
  const pct = u?.fiveHour || u?.sevenDay ? ` — 5h ${Math.round(u.fiveHour?.pct ?? 0)}% · 7d ${Math.round(u.sevenDay?.pct ?? 0)}%` : a.usageStatus !== 'ok' ? ` — ${a.usageStatus}` : ''
  return `${a.active ? '● ' : '   '}${a.number}. ${who}${pct}${a.disabled ? ' (disabled)' : ''}`
}

export function createTray(core: AppCore, getWindow: () => BrowserWindow | null, show: () => void, quit: () => void): Tray | null {
  // macOS wants a template (black on transparent, tinted by the OS); Windows and Linux
  // get the white glyph with a dark outline, which survives a light *and* a dark tray.
  const file = process.platform === 'darwin' ? 'trayTemplate.png' : 'tray.png'
  let icon = nativeImage.createFromPath(join(__dirname, '../../resources', file))
  if (icon.isEmpty()) icon = nativeImage.createFromPath(join(process.resourcesPath ?? '', file))
  if (icon.isEmpty()) return null
  if (process.platform === 'darwin') icon.setTemplateImage(true)
  const tray = new Tray(icon)
  tray.setToolTip('Claude Swap')

  const rebuild = (): void => {
    const state = core.getAccounts()
    const accounts = state.payload?.accounts ?? []
    const active = accounts.find((a) => a.active)
    const auto = core.auto.getStatus()
    tray.setToolTip(
      active
        ? `Claude Swap — Account-${active.number} ${active.alias || active.email}` +
            (active.usage?.fiveHour ? `\n5h ${Math.round(active.usage.fiveHour.pct)}% · 7d ${Math.round(active.usage.sevenDay?.pct ?? 0)}%` : '')
        : 'Claude Swap — no active account'
    )
    const menu = Menu.buildFromTemplate([
      { label: 'Open Claude Swap', click: show },
      { type: 'separator' },
      ...accounts.map((a) => ({
        label: label(a),
        enabled: !a.active,
        click: () => void core.driver.switchTo(String(a.number)).then((r) => core.afterMutation(r))
      })),
      ...(accounts.length ? [{ type: 'separator' as const }] : []),
      { label: 'Rotate to next', enabled: accounts.length > 1, click: () => void core.driver.rotate().then((r) => core.afterMutation(r)) },
      { label: 'Switch to best', enabled: accounts.length > 1, click: () => void core.driver.rotate('best').then((r) => core.afterMutation(r)) },
      { label: 'Refresh usage', click: () => void core.refresh() },
      { type: 'separator' },
      {
        label: auto.running ? `Stop auto-switch${auto.dryRun ? ' (dry run)' : ''}` : 'Start auto-switch',
        click: () => (auto.running ? void core.auto.stop() : core.auto.start(core.driver.getBinary(), { dryRun: core.settings.get().autoDryRun }))
      },
      { type: 'separator' },
      { label: 'Quit', click: quit }
    ])
    tray.setContextMenu(menu)
  }
  rebuild()
  core.on('accounts', rebuild)
  core.on('autoStatus', rebuild)
  tray.on('click', () => {
    const w = getWindow()
    if (w && w.isVisible() && !w.isMinimized() && process.platform !== 'darwin') w.hide()
    else show()
  })
  tray.on('double-click', show)
  return tray
}
