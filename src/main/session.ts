import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import type { Result } from '@shared/types'

// Opens a terminal window running `cswap run <target>` — session mode, that terminal
// only. Best effort per platform; the command is returned so the UI can show it.
// The terminal starts where the caller asks — a mapped project, or home — never
// wherever the app itself happens to have been started from.
export function launchSession(bin: string | null, target: string, cwd?: string): Result<{ command: string; cwd: string }> {
  if (!bin) return { ok: false, error: { type: 'NotFound', message: 'cswap binary not configured' } }
  const dir = cwd && cwd.trim() ? cwd : homedir()
  if (!existsSync(dir)) return { ok: false, error: { type: 'NotFound', message: `No such directory: ${dir}` } }
  const command = `cswap run ${target}`
  // e2e seam: the wiring can be checked without a terminal actually appearing on the
  // machine running the tests. It proves the arguments, never the spawn.
  if (process.env.CSWAP_DESKTOP_TEST_NO_SPAWN) return { ok: true, value: { command, cwd: dir } }
  try {
    if (process.platform === 'win32') {
      // detached → its own console window. Verbatim args: cmd does not understand the
      // backslash-escaped quotes Node would otherwise produce; /s strips the outer pair.
      const line = `"title cswap run ${target}& "${bin}" run ${target}"`
      spawn('cmd.exe', ['/d', '/s', '/k', line], { detached: true, stdio: 'ignore', windowsHide: false, windowsVerbatimArguments: true, cwd: dir }).unref()
    } else if (process.platform === 'darwin') {
      // Terminal.app opens in its own default directory, so the cd belongs in the
      // command itself; the spawn cwd only affects osascript.
      const q = (v: string): string => v.replace(/(["\\$`])/g, '\\$1')
      const script = `tell application "Terminal" to do script "cd \\"${q(dir)}\\" && \\"${q(bin)}\\" run ${target}"`
      spawn('osascript', ['-e', script, '-e', 'tell application "Terminal" to activate'], { detached: true, stdio: 'ignore', cwd: dir }).unref()
    } else {
      const cmd = `cd ${JSON.stringify(dir)} && ${JSON.stringify(bin)} run ${target}; exec $SHELL`
      const terminals: [string, string[]][] = [
        ['x-terminal-emulator', ['-e', `bash -lc '${cmd}'`]],
        ['gnome-terminal', ['--', 'bash', '-lc', cmd]],
        ['konsole', ['-e', 'bash', '-lc', cmd]],
        ['xterm', ['-e', 'bash', '-lc', cmd]]
      ]
      let launched = false
      for (const [t, args] of terminals) {
        try {
          const child = spawn(t, args, { detached: true, stdio: 'ignore', cwd: dir })
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
    return { ok: true, value: { command, cwd: dir } }
  } catch (e) {
    return { ok: false, error: { type: 'LaunchError', message: (e as Error).message } }
  }
}
