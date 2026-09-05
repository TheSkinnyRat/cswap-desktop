import { BrowserWindow, Menu, app, nativeTheme, shell, type Tray } from 'electron'
import { join } from 'node:path'
import { AppCore } from './core'
import { registerIpc } from './ipc'
import { createTray } from './tray'
import { IPC } from '@shared/types'

// Tests point these at a scratch directory + a fake cswap.
if (process.env.CSWAP_DESKTOP_USERDATA) app.setPath('userData', process.env.CSWAP_DESKTOP_USERDATA)

let win: BrowserWindow | null = null
let tray: Tray | null = null
let core: AppCore
let quitting = false

const gotLock = app.requestSingleInstanceLock()
if (!gotLock) app.quit()

function createWindow(): BrowserWindow {
  const w = new BrowserWindow({
    width: 1120,
    height: 720,
    minWidth: 760,
    minHeight: 520,
    show: false,
    title: 'cswap',
    backgroundColor: nativeTheme.shouldUseDarkColors ? '#151517' : '#f4f4f6',
    autoHideMenuBar: true,
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'hidden',
    titleBarOverlay: process.platform === 'darwin' ? undefined : { color: '#00000000', symbolColor: '#8a8a95', height: 38 },
    trafficLightPosition: { x: 14, y: 12 },
    icon: join(__dirname, '../../resources/icon.png'),
    webPreferences: {
      preload: join(__dirname, '../preload/index.mjs'),
      sandbox: false,
      contextIsolation: true,
      nodeIntegration: false
    }
  })
  w.on('ready-to-show', () => {
    if (!(process.argv.includes('--hidden') || core.settings.get().startMinimized)) w.show()
  })
  w.on('close', (e) => {
    if (!quitting && core.settings.get().closeToTray && tray) {
      e.preventDefault()
      w.hide()
    }
  })
  w.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:\/\//.test(url)) void shell.openExternal(url)
    return { action: 'deny' }
  })
  if (process.env.ELECTRON_RENDERER_URL) void w.loadURL(process.env.ELECTRON_RENDERER_URL)
  else void w.loadFile(join(__dirname, '../renderer/index.html'))
  return w
}

function show(): void {
  if (!win || win.isDestroyed()) win = createWindow()
  if (win.isMinimized()) win.restore()
  win.show()
  win.focus()
}

app.on('second-instance', show)

app.whenReady().then(async () => {
  Menu.setApplicationMenu(null)
  core = new AppCore(app.getPath('userData'))
  registerIpc(core, () => win)
  win = createWindow()
  tray = createTray(core, () => win, show, () => {
    quitting = true
    app.quit()
  })
  await core.init()
  core.on('settings', (s) => {
    nativeTheme.themeSource = s.theme
  })
  nativeTheme.themeSource = core.settings.get().theme
  win.webContents.on('did-finish-load', () => win?.webContents.send(IPC.evSettings, core.settings.get()))
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin' && !tray) app.quit()
})
app.on('activate', show)
app.on('before-quit', () => {
  quitting = true
  core?.dispose()
})
