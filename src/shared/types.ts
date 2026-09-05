// Contract shared by main, preload and renderer. Everything cswap-shaped here
// mirrors claude-swap's `--json` schema v1 (additive: unknown fields are kept).

export type UsageStatus =
  | 'ok'
  | 'token_expired'
  | 'api_key'
  | 'keychain_unavailable'
  | 'relogin_required'
  | 'foreign_credential'
  | 'no_credentials'
  | 'unavailable'
  | (string & {})

export interface UsageWindow {
  pct: number
  resetsAt?: string
  countdown?: string
  clock?: string
  // weekly windows only, present once the week is ~a day old
  expectedPct?: number
  aheadOfPace?: boolean
  projectedExhaustionAt?: string
  willLastToReset?: boolean
}

export interface ScopedWindow extends UsageWindow {
  name: string
}

export interface Usage {
  fiveHour?: UsageWindow
  sevenDay?: UsageWindow
  scoped?: ScopedWindow[]
}

export interface Account {
  number: number
  email: string
  organizationName: string
  organizationUuid: string
  isOrganization: boolean
  active: boolean
  usageStatus: UsageStatus
  usage: Usage | null
  alias?: string
  disabled?: boolean
  usageFetchedAt?: string
  usageAgeSeconds?: number
  lastGoodUsage?: Usage
  lastGoodFetchedAt?: string
  lastGoodAgeSeconds?: number
}

export interface ListPayload {
  schemaVersion: number
  activeAccountNumber: number | null
  accounts: Account[]
}

export interface AccountRef {
  number: number | null
  email: string
}

export interface SwitchPayload {
  schemaVersion: number
  switched: boolean
  from: AccountRef | null
  to: AccountRef | null
  strategy: string
  reason: string
  message: string
  warnings: string[]
  models?: string[]
  modelSource?: string
}

export interface StatusActive extends Partial<Account> {
  email: string
  managed: boolean
}

export interface StatusPayload {
  schemaVersion: number
  active: StatusActive | null
  totalManagedAccounts?: number
}

export interface ConfigSetting {
  key: string
  value: unknown
  isSet: boolean
}

export interface ConfigPayload {
  schemaVersion: number
  path: string
  settings: ConfigSetting[]
}

export interface CswapError {
  type: string
  message: string
}

// One line of `cswap auto --json`. Kinds are additive upstream; keep the union open.
export interface AutoEvent {
  schemaVersion: number
  event:
    | 'poll'
    | 'switch'
    | 'no-switch'
    | 'account-quarantined'
    | 'account-unquarantined'
    | 'all-exhausted'
    | 'sleep'
    | 'error'
    | 'config-warning'
    | (string & {})
  ts: string
  [k: string]: unknown
}

export interface AutoStatus {
  running: boolean
  pid?: number
  startedAt?: string
  dryRun?: boolean
  exitCode?: number | null
  lastError?: string
}

export interface Mapping {
  path: string
  email: string
  organizationUuid?: string
  account?: Account
}

export interface UnclaimedEntry {
  id: string
  slot: string
  reason: string
}

// A single CLI invocation as the app saw it — the LOG page shows these.
export interface CommandLogEntry {
  id: number
  at: string
  args: string[]
  exitCode: number | null
  durationMs: number
  ok: boolean
  stdout?: string
  stderr?: string
  error?: string
}

export interface CswapBinaryInfo {
  path: string | null
  source: 'settings' | 'uv' | 'pipx' | 'path' | 'none'
  version: string | null
  error?: string
  candidates: string[]
}

export type ThemeSetting = 'light' | 'dark' | 'system'

export interface AppSettings {
  cswapPath: string | null
  theme: ThemeSetting
  refreshSeconds: number
  closeToTray: boolean
  startMinimized: boolean
  launchAtLogin: boolean
  notifyOnAutoSwitch: boolean
  autoStartAutoSwitch: boolean
  autoDryRun: boolean
}

export const DEFAULT_SETTINGS: AppSettings = {
  cswapPath: null,
  theme: 'light',
  refreshSeconds: 60,
  closeToTray: true,
  startMinimized: false,
  launchAtLogin: false,
  notifyOnAutoSwitch: true,
  autoStartAutoSwitch: false,
  autoDryRun: false
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: CswapError }

export interface AccountsState {
  payload: ListPayload | null
  fetchedAt: string | null
  refreshing: boolean
  error: CswapError | null
}

export interface ExportOptions {
  account?: string
  full?: boolean
}

export interface ImportOptions {
  force?: boolean
}

export interface AddAccountOptions {
  slot?: number
  alias?: string
  overwrite?: boolean
}

export interface AddTokenOptions {
  token: string
  email?: string
  slot?: number
  overwrite?: boolean
}

export interface PlainOutput {
  stdout: string
  stderr: string
  exitCode: number | null
}

// What the renderer can call. Implemented in preload over ipcRenderer.invoke.
export interface CswapApi {
  // state
  getAccounts(): Promise<AccountsState>
  refresh(): Promise<AccountsState>
  getStatus(): Promise<Result<StatusPayload>>
  // mutations (all go through the cswap CLI)
  switchTo(target: string, force?: boolean): Promise<Result<SwitchPayload>>
  rotate(strategy?: 'best' | 'next-available'): Promise<Result<SwitchPayload>>
  addAccount(opts: AddAccountOptions): Promise<Result<PlainOutput>>
  addToken(opts: AddTokenOptions): Promise<Result<PlainOutput>>
  removeAccount(target: string): Promise<Result<PlainOutput>>
  setDisabled(target: string, disabled: boolean): Promise<Result<PlainOutput>>
  setAlias(target: string, alias: string | null): Promise<Result<PlainOutput>>
  moveAccount(target: string, slot: number): Promise<Result<PlainOutput>>
  swapAccounts(a: string, b: string): Promise<Result<PlainOutput>>
  // mappings
  listMappings(): Promise<Result<Mapping[]>>
  mapDirectory(target: string, path: string): Promise<Result<PlainOutput>>
  unmapDirectory(path: string): Promise<Result<PlainOutput>>
  pickDirectory(): Promise<string | null>
  // config (cswap settings.json)
  getConfig(): Promise<Result<ConfigPayload>>
  setConfig(key: string, value: string | null): Promise<Result<PlainOutput>>
  // transfer
  exportAccounts(opts: ExportOptions): Promise<Result<{ path: string; output: PlainOutput } | null>>
  importAccounts(opts: ImportOptions): Promise<Result<{ path: string; output: PlainOutput } | null>>
  // maintenance
  listUnclaimed(): Promise<Result<UnclaimedEntry[]>>
  purgeUnclaimed(id: string): Promise<Result<PlainOutput>>
  upgradeCswap(): Promise<Result<PlainOutput>>
  // auto-switch process
  autoStart(opts?: { dryRun?: boolean }): Promise<AutoStatus>
  autoStop(): Promise<AutoStatus>
  autoStatus(): Promise<AutoStatus>
  autoEvents(): Promise<AutoEvent[]>
  // app
  getSettings(): Promise<AppSettings>
  setSettings(patch: Partial<AppSettings>): Promise<AppSettings>
  getBinaryInfo(): Promise<CswapBinaryInfo>
  detectBinary(): Promise<CswapBinaryInfo>
  pickBinary(): Promise<string | null>
  installCswap(): Promise<Result<PlainOutput>>
  getCommandLog(): Promise<CommandLogEntry[]>
  clearCommandLog(): Promise<void>
  getVaultPath(): Promise<string | null>
  openPath(path: string): Promise<void>
  openExternal(url: string): Promise<void>
  getAppInfo(): Promise<{ version: string; platform: NodeJS.Platform; electron: string }>
  windowControl(action: 'minimize' | 'maximize' | 'close' | 'hide'): Promise<void>
  // push events
  onAccounts(cb: (state: AccountsState) => void): () => void
  onAutoEvent(cb: (ev: AutoEvent) => void): () => void
  onAutoStatus(cb: (st: AutoStatus) => void): () => void
  onCommandLog(cb: (entry: CommandLogEntry) => void): () => void
  onSettings(cb: (s: AppSettings) => void): () => void
  onNavigate(cb: (page: string) => void): () => void
}

export const IPC = {
  getAccounts: 'cswap:getAccounts',
  refresh: 'cswap:refresh',
  getStatus: 'cswap:getStatus',
  switchTo: 'cswap:switchTo',
  rotate: 'cswap:rotate',
  addAccount: 'cswap:addAccount',
  addToken: 'cswap:addToken',
  removeAccount: 'cswap:removeAccount',
  setDisabled: 'cswap:setDisabled',
  setAlias: 'cswap:setAlias',
  moveAccount: 'cswap:moveAccount',
  swapAccounts: 'cswap:swapAccounts',
  listMappings: 'cswap:listMappings',
  mapDirectory: 'cswap:mapDirectory',
  unmapDirectory: 'cswap:unmapDirectory',
  pickDirectory: 'app:pickDirectory',
  getConfig: 'cswap:getConfig',
  setConfig: 'cswap:setConfig',
  exportAccounts: 'cswap:export',
  importAccounts: 'cswap:import',
  listUnclaimed: 'cswap:listUnclaimed',
  purgeUnclaimed: 'cswap:purgeUnclaimed',
  upgradeCswap: 'cswap:upgrade',
  autoStart: 'auto:start',
  autoStop: 'auto:stop',
  autoStatus: 'auto:status',
  autoEvents: 'auto:events',
  getSettings: 'app:getSettings',
  setSettings: 'app:setSettings',
  getBinaryInfo: 'app:getBinaryInfo',
  detectBinary: 'app:detectBinary',
  pickBinary: 'app:pickBinary',
  installCswap: 'app:installCswap',
  getCommandLog: 'app:getCommandLog',
  clearCommandLog: 'app:clearCommandLog',
  getVaultPath: 'app:getVaultPath',
  openPath: 'app:openPath',
  openExternal: 'app:openExternal',
  getAppInfo: 'app:getAppInfo',
  windowControl: 'app:windowControl',
  // push
  evAccounts: 'ev:accounts',
  evAutoEvent: 'ev:autoEvent',
  evAutoStatus: 'ev:autoStatus',
  evCommandLog: 'ev:commandLog',
  evSettings: 'ev:settings',
  evNavigate: 'ev:navigate'
} as const
