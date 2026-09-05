import { execFile } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { join, delimiter } from 'node:path'
import type { CswapBinaryInfo } from '@shared/types'

const isWin = process.platform === 'win32'
const exe = isWin ? 'cswap.exe' : 'cswap'

// Where uv / pipx put the shim, per platform. Checked before PATH because an app
// launched from the Start menu or Finder gets the login PATH, not the shell's.
export function candidatePaths(): string[] {
  const home = homedir()
  const c: string[] = [join(home, '.local', 'bin', exe)]
  if (isWin) {
    const appData = process.env.APPDATA
    const localAppData = process.env.LOCALAPPDATA
    if (appData) c.push(join(appData, 'Python', 'Scripts', exe))
    if (localAppData) c.push(join(localAppData, 'Programs', 'Python', 'Scripts', exe))
  } else {
    c.push('/opt/homebrew/bin/cswap', '/usr/local/bin/cswap', join(home, '.pipx', 'bin', 'cswap'))
  }
  return c
}

function findOnPath(pathVar: string | undefined): string | null {
  if (!pathVar) return null
  for (const dir of pathVar.split(delimiter)) {
    if (!dir) continue
    const p = join(dir, exe)
    if (existsSync(p)) return p
  }
  return null
}

// macOS Finder-launched apps miss the shell PATH entirely; ask the login shell once.
async function loginShellPath(): Promise<string | null> {
  if (isWin || process.platform === 'linux') return null
  const shell = process.env.SHELL || '/bin/zsh'
  return new Promise((resolve) => {
    execFile(shell, ['-ilc', 'echo -n "$PATH"'], { timeout: 5000 }, (err, stdout) => {
      resolve(err ? null : stdout.trim() || null)
    })
  })
}

export function runVersion(bin: string): Promise<{ version: string | null; error?: string }> {
  return new Promise((resolve) => {
    const useShell = isWin && /\.(cmd|bat)$/i.test(bin)
    execFile(
      useShell ? `"${bin}"` : bin,
      ['--version'],
      { timeout: 15000, windowsHide: true, shell: useShell, env: { ...process.env, NO_COLOR: '1' } },
      (err, stdout, stderr) => {
        if (err) return resolve({ version: null, error: (stderr || err.message).trim() })
        const m = String(stdout).match(/(\d+\.\d+\.\d+\S*)/)
        resolve({ version: m ? m[1] : String(stdout).trim() || null })
      }
    )
  })
}

export async function resolveCswap(settingsPath: string | null): Promise<CswapBinaryInfo> {
  const candidates = candidatePaths()
  const envOverride = process.env.CSWAP_DESKTOP_BIN || null

  const tryBin = async (
    path: string,
    source: CswapBinaryInfo['source']
  ): Promise<CswapBinaryInfo | null> => {
    const { version, error } = await runVersion(path)
    if (version) return { path, source, version, candidates }
    return { path, source, version: null, error, candidates }
  }

  // Precedence: a path chosen in Settings beats the CSWAP_DESKTOP_BIN environment
  // override (used by tests / power users), which beats auto-detection.
  if (settingsPath) {
    if (existsSync(settingsPath) || /\.(cmd|bat)$/i.test(settingsPath)) {
      const r = await tryBin(settingsPath, 'settings')
      if (r) return r
    } else {
      return { path: settingsPath, source: 'settings', version: null, error: 'File not found', candidates }
    }
  }
  if (envOverride) {
    const r = await tryBin(envOverride, 'settings')
    if (r) return r
  }
  for (const p of candidates) {
    if (!existsSync(p)) continue
    const r = await tryBin(p, p.includes('pipx') ? 'pipx' : 'uv')
    if (r?.version) return r
  }
  const onPath = findOnPath(process.env.PATH) || findOnPath((await loginShellPath()) ?? undefined)
  if (onPath) {
    const r = await tryBin(onPath, 'path')
    if (r?.version) return r
  }
  return { path: null, source: 'none', version: null, candidates }
}

export function findUv(): string | null {
  const home = homedir()
  const names = isWin ? ['uv.exe'] : ['uv']
  const dirs = [join(home, '.local', 'bin'), join(home, '.cargo', 'bin'), '/opt/homebrew/bin', '/usr/local/bin']
  for (const d of dirs) for (const n of names) if (existsSync(join(d, n))) return join(d, n)
  return findOnPath(process.env.PATH)?.replace(exe, names[0]) ?? null
}
