import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { EventEmitter } from 'node:events'
import { DEFAULT_SETTINGS, type AppSettings } from '@shared/types'

export class SettingsStore extends EventEmitter {
  private file: string
  private data: AppSettings

  constructor(userData: string) {
    super()
    this.file = join(userData, 'settings.json')
    this.data = this.load()
  }

  private load(): AppSettings {
    try {
      if (existsSync(this.file)) {
        const raw = JSON.parse(readFileSync(this.file, 'utf8')) as Partial<AppSettings>
        return { ...DEFAULT_SETTINGS, ...raw }
      }
    } catch {
      /* corrupt file → defaults */
    }
    return { ...DEFAULT_SETTINGS }
  }

  get(): AppSettings {
    return { ...this.data }
  }

  set(patch: Partial<AppSettings>): AppSettings {
    this.data = { ...this.data, ...patch }
    mkdirSync(dirname(this.file), { recursive: true })
    const tmp = `${this.file}.tmp`
    writeFileSync(tmp, JSON.stringify(this.data, null, 2))
    renameSync(tmp, this.file)
    this.emit('change', this.get())
    return this.get()
  }
}
