import { spawn, type ChildProcess } from 'node:child_process'
import { EventEmitter } from 'node:events'
import type { AutoEvent, AutoStatus } from '@shared/types'

const isWin = process.platform === 'win32'

// Owns the long-running `cswap auto --json` child. One at a time; events are
// parsed line by line and kept in a ring buffer for the AUTO page.
export class AutoSwitchRunner extends EventEmitter {
  private child: ChildProcess | null = null
  private status: AutoStatus = { running: false }
  private events: AutoEvent[] = []
  private buf = ''

  getStatus(): AutoStatus {
    return { ...this.status }
  }
  getEvents(): AutoEvent[] {
    return this.events
  }

  start(bin: string | null, opts: { dryRun?: boolean } = {}): AutoStatus {
    if (this.child) return this.getStatus()
    if (!bin) {
      this.status = { running: false, lastError: 'cswap binary not configured' }
      this.emit('status', this.getStatus())
      return this.getStatus()
    }
    const args = ['auto', '--json']
    if (opts.dryRun) args.push('--dry-run')
    const useShell = isWin && /\.(cmd|bat)$/i.test(bin)
    const env = { ...process.env, NO_COLOR: '1', PYTHONIOENCODING: 'utf-8', PYTHONUTF8: '1', PYTHONUNBUFFERED: '1' }
    const child = useShell
      ? spawn([`"${bin}"`, ...args].join(' '), { shell: true, windowsHide: true, env })
      : spawn(bin, args, { windowsHide: true, env })
    this.child = child
    this.buf = ''
    this.status = { running: true, pid: child.pid, startedAt: new Date().toISOString(), dryRun: !!opts.dryRun, exitCode: null }
    child.stdout?.setEncoding('utf8')
    child.stderr?.setEncoding('utf8')
    child.stdout?.on('data', (d: string) => this.onData(d))
    child.stderr?.on('data', (d: string) => {
      const msg = d.trim()
      if (msg) this.push({ schemaVersion: 1, event: 'stderr', ts: new Date().toISOString(), message: msg })
    })
    child.on('error', (err) => {
      this.status = { running: false, exitCode: null, lastError: err.message }
      this.child = null
      this.emit('status', this.getStatus())
    })
    child.on('close', (code) => {
      this.flush()
      this.status = { ...this.status, running: false, exitCode: code, pid: undefined }
      this.child = null
      this.emit('status', this.getStatus())
    })
    this.emit('status', this.getStatus())
    return this.getStatus()
  }

  stop(): Promise<AutoStatus> {
    const child = this.child
    if (!child) return Promise.resolve(this.getStatus())
    return new Promise((resolve) => {
      const done = (): void => resolve(this.getStatus())
      child.once('close', () => setTimeout(done, 0))
      // cswap handles SIGTERM cleanly on POSIX; Windows has no signal, so kill.
      if (isWin && child.pid) spawn('taskkill', ['/pid', String(child.pid), '/t', '/f'], { windowsHide: true })
      else child.kill('SIGTERM')
      setTimeout(() => {
        if (this.child === child) child.kill('SIGKILL')
      }, 5000)
    })
  }

  private onData(d: string): void {
    this.buf += d
    let idx: number
    while ((idx = this.buf.indexOf('\n')) >= 0) {
      const line = this.buf.slice(0, idx).trim()
      this.buf = this.buf.slice(idx + 1)
      if (line) this.parseLine(line)
    }
  }
  private flush(): void {
    const rest = this.buf.trim()
    this.buf = ''
    if (rest) this.parseLine(rest)
  }
  private parseLine(line: string): void {
    try {
      const obj = JSON.parse(line) as AutoEvent & { error?: { message: string } }
      if (obj.error) {
        this.push({ schemaVersion: 1, event: 'error', ts: new Date().toISOString(), message: obj.error.message, transient: false })
        return
      }
      if (!obj.ts) obj.ts = new Date().toISOString()
      this.push(obj)
    } catch {
      this.push({ schemaVersion: 1, event: 'raw', ts: new Date().toISOString(), message: line })
    }
  }
  private push(ev: AutoEvent): void {
    this.events.push(ev)
    if (this.events.length > 500) this.events.splice(0, this.events.length - 500)
    this.emit('event', ev)
  }
}
