import { existsSync, readFileSync, watch, type FSWatcher } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'
import { EventEmitter } from 'node:events'

// The claude-swap "backup root": sequence.json, settings.json, mappings.json,
// autoswitch_state.json live here. Read-only from our side — every write goes
// through the CLI — but watching it keeps the UI in sync with the terminal.
export function defaultVaultPath(): string {
  if (process.platform === 'linux') {
    const xdg = process.env.XDG_DATA_HOME
    return join(xdg && xdg.trim() ? xdg : join(homedir(), '.local', 'share'), 'claude-swap')
  }
  return join(homedir(), '.claude-swap-backup')
}

export function vaultFromSettingsPath(settingsJson: string | null): string | null {
  return settingsJson ? dirname(settingsJson) : null
}

export interface MappingsFile {
  [path: string]: { email?: string; organizationUuid?: string }
}

export function readMappingsFile(vault: string): MappingsFile | null {
  const p = join(vault, 'mappings.json')
  if (!existsSync(p)) return null
  try {
    const raw = JSON.parse(readFileSync(p, 'utf8')) as unknown
    if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
      const obj = raw as Record<string, unknown>
      // Either { mappings: {...} } or a flat map, depending on the version.
      const inner = (obj.mappings && typeof obj.mappings === 'object' ? obj.mappings : obj) as MappingsFile
      const out: MappingsFile = {}
      for (const [k, v] of Object.entries(inner)) if (v && typeof v === 'object') out[k] = v as MappingsFile[string]
      return out
    }
  } catch {
    /* unreadable → treat as none */
  }
  return null
}

const INTERESTING = new Set(['sequence.json', 'settings.json', 'mappings.json', 'autoswitch_state.json'])

export class VaultWatcher extends EventEmitter {
  private watcher: FSWatcher | null = null
  private timer: NodeJS.Timeout | null = null
  private path: string | null = null

  watch(path: string | null): void {
    this.close()
    this.path = path
    if (!path || !existsSync(path)) return
    try {
      this.watcher = watch(path, { persistent: false }, (_ev, file) => {
        const name = typeof file === 'string' ? file : file ? String(file) : ''
        if (name && !INTERESTING.has(name)) return
        if (this.timer) clearTimeout(this.timer)
        this.timer = setTimeout(() => this.emit('change', name), 600)
      })
      this.watcher.on('error', () => this.close())
    } catch {
      this.watcher = null
    }
  }
  current(): string | null {
    return this.path
  }
  close(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    this.watcher?.close()
    this.watcher = null
  }
}
