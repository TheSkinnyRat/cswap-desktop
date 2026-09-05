import type { Account, UsageStatus, UsageWindow } from '@shared/types'

export function pct(n: number | undefined | null): string {
  if (n === undefined || n === null || Number.isNaN(n)) return '—'
  return `${Math.round(n)}%`
}

export function relTime(iso: string | undefined | null, now = Date.now()): string {
  if (!iso) return ''
  const t = new Date(iso).getTime()
  if (Number.isNaN(t)) return ''
  const diff = Math.round((t - now) / 1000)
  const abs = Math.abs(diff)
  const s = abs < 60 ? `${abs}s` : abs < 3600 ? `${Math.floor(abs / 60)}m` : abs < 86400 ? `${Math.floor(abs / 3600)}h ${Math.floor((abs % 3600) / 60)}m` : `${Math.floor(abs / 86400)}d ${Math.floor((abs % 86400) / 3600)}h`
  return diff >= 0 ? `in ${s}` : `${s} ago`
}

export function ageSeconds(sec: number | undefined): string {
  if (sec === undefined) return ''
  if (sec < 60) return 'just now'
  if (sec < 3600) return `${Math.floor(sec / 60)}m ago`
  if (sec < 86400) return `${Math.floor(sec / 3600)}h ago`
  return `${Math.floor(sec / 86400)}d ago`
}

// "3h 30m left" for a future reset; a reset already in the past (stale last-good
// data) says so instead of printing "41d ago left".
export function resetText(iso: string | undefined, now = Date.now()): string {
  if (!iso) return ''
  const t = new Date(iso).getTime()
  if (Number.isNaN(t)) return ''
  if (t <= now) return 'reset passed'
  return `${relTime(iso, now).replace(/^in /, '')} left`
}

export function clockOf(iso: string | undefined): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const sameDay = d.toDateString() === new Date().toDateString()
  const hm = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  return sameDay ? hm : `${d.toLocaleDateString([], { month: 'short', day: 'numeric' })} ${hm}`
}

export const STATUS_LABEL: Record<string, string> = {
  ok: 'OK',
  token_expired: 'Token expired',
  api_key: 'API key',
  keychain_unavailable: 'Keychain locked',
  relogin_required: 'Re-login required',
  foreign_credential: 'Foreign credential',
  no_credentials: 'No credentials',
  unavailable: 'Usage unavailable'
}

export function statusLabel(s: UsageStatus): string {
  return STATUS_LABEL[s] ?? s
}

export function statusTone(s: UsageStatus): 'ok' | 'warn' | 'danger' | 'muted' {
  if (s === 'ok') return 'ok'
  if (s === 'api_key' || s === 'unavailable') return 'muted'
  if (s === 'token_expired' || s === 'relogin_required' || s === 'no_credentials') return 'danger'
  return 'warn'
}

export function tone(p: number | undefined): 'ok' | 'warn' | 'danger' | 'muted' {
  if (p === undefined) return 'muted'
  if (p >= 100) return 'danger'
  if (p >= 80) return 'warn'
  return 'ok'
}

export function displayName(a: Account): string {
  return a.alias || a.email
}

export function orgTag(a: Account): string {
  if (!a.isOrganization) return 'personal'
  const name = a.organizationName || ''
  // Anthropic names a personal workspace "<email>'s Organization" — that is not a team.
  if (!name || /'s Organization$/.test(name)) return 'personal'
  return name
}

// Which window binds (highest utilization) — same idea cswap uses for "binding".
export function binding(a: Account): { label: string; window: UsageWindow } | null {
  const u = a.usage ?? a.lastGoodUsage
  if (!u) return null
  const all: { label: string; window: UsageWindow }[] = []
  if (u.fiveHour) all.push({ label: '5h', window: u.fiveHour })
  if (u.sevenDay) all.push({ label: '7d', window: u.sevenDay })
  for (const s of u.scoped ?? []) all.push({ label: s.name, window: s })
  if (!all.length) return null
  return all.reduce((m, x) => (x.window.pct > m.window.pct ? x : m))
}

export function shortArgs(args: string[]): string {
  return args.join(' ')
}
