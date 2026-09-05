import { spawn } from 'node:child_process'
import type { Result } from '@shared/types'

// Opens a terminal window running `cswap run <target>` — session mode, that terminal
// only. Best effort per platform; the command is returned so the UI can show it.
export function launchSession(bin: string | null, target: string): Result<{ command: string }> {
  if (!bin) return { ok: false, error: { type: 'NotFound', message: 'cswap binary not configured' } }
  const command = `cswap run ${target}`
  try {
    if (process.platform === 'win32') {
      // detached → its own console window. Verbatim args: cmd does not understand the
      // backslash-escaped quotes Node would otherwise produce; /s strips the outer pair.
      const line = `"title cswap run ${target}& "${bin}" run ${target}"`
      spawn('cmd.exe', ['/d', '/s', '/k', line], { detached: true, stdio: 'ignore', windowsHide: false, windowsVerbatimArguments: true }).unref()
    } else if (process.platform === 'darwin') {
      const script = `tell application "Terminal" to do script "${bin.replace(/"/g, '\\"')} run ${target}"`
      spawn('osascript', ['-e', script, '-e', 'tell application "Terminal" to activate'], { detached: true, stdio: 'ignore' }).unref()
    } else {
      const cmd = `${JSON.stringify(bin)} run ${target}; exec $SHELL`
      const terminals: [string, string[]][] = [
        ['x-terminal-emulator', ['-e', `bash -lc '${cmd}'`]],
        ['gnome-terminal', ['--', 'bash', '-lc', cmd]],
        ['konsole', ['-e', 'bash', '-lc', cmd]],
        ['xterm', ['-e', 'bash', '-lc', cmd]]
      ]
      let launched = false
      for (const [t, args] of terminals) {
        try {
          const child = spawn(t, args, { detached: true, stdio: 'ignore' })
          child.on('error', () => {})
          child.unref()
          launched = true
          break
        } catch {
          /* try the next terminal */
        }
      }
      if (!launched) return { ok: false, error: { type: 'NotFound', message: 'No terminal emulator found' } }
    }
    return { ok: true, value: { command } }
  } catch (e) {
    return { ok: false, error: { type: 'LaunchError', message: (e as Error).message } }
  }
}
