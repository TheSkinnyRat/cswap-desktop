import { useEffect, useMemo, useRef, useState } from 'react'
import { Activity, Pause, Play, Zap } from 'lucide-react'
import type { AutoEvent, ConfigSetting } from '@shared/types'
import { useStore } from '../lib/store'
import { api } from '../lib/api'
import { useToast } from '../lib/toast'
import { Button, Card, Chip, Input, Select, Switch, cx } from '../components/ui'
import { PageHeader } from '../components/PageHeader'
import { clockOf, relTime } from '../lib/format'

const KIND_TONE: Record<string, 'ok' | 'warn' | 'danger' | 'muted' | 'accent'> = {
  switch: 'accent',
  poll: 'muted',
  sleep: 'muted',
  'no-switch': 'muted',
  error: 'danger',
  stderr: 'warn',
  'account-quarantined': 'warn',
  'account-unquarantined': 'ok',
  'all-exhausted': 'danger',
  'config-warning': 'warn',
  raw: 'muted'
}

function describe(ev: AutoEvent): string {
  const s = (v: unknown): string => (typeof v === 'string' ? v : v == null ? '' : JSON.stringify(v))
  const ref = (r: unknown): string => {
    const x = r as { number?: number; email?: string } | null
    return x ? `Account-${x.number ?? '?'} (${x.email ?? ''})` : '—'
  }
  switch (ev.event) {
    case 'poll': {
      const w = ev.windowsPct as Record<string, Record<string, number>> | undefined
      const h = ev.headroomPct as Record<string, number | null> | undefined
      const parts = Object.entries(w ?? {}).map(([n, wins]) => `#${n} ${Object.entries(wins).map(([k, v]) => `${k} ${Math.round(v)}%`).join(' · ')}`)
      if (!parts.length && h) parts.push(...Object.entries(h).map(([n, v]) => `#${n} ${v == null ? '?' : `${Math.round(100 - v)}%`}`))
      return `active ${ref(ev.active)} · threshold ${s(ev.threshold)}% · ${parts.join('  |  ')}`
    }
    case 'switch':
      return `${ev.dryRun ? '[dry run] would switch' : 'switched'} ${ref(ev.from)} → ${ref(ev.to)} · ${s(ev.trigger)}${(ev.warnings as string[])?.length ? ` · ${(ev.warnings as string[]).join('; ')}` : ''}`
    case 'no-switch':
      return `${s(ev.reason)}${ev.detail ? ` — ${s(ev.detail)}` : ''}`
    case 'sleep':
      return `next check in ${s(ev.seconds)}s (${clockOf(s(ev.until))})`
    case 'account-quarantined':
    case 'account-unquarantined':
      return `Account-${s(ev.number)} (${s(ev.email)}) · ${s(ev.reason)}`
    case 'all-exhausted':
      return `every account is at its limit · earliest reset ${ev.earliestResetAt ? relTime(s(ev.earliestResetAt)) : 'unknown'}`
    default:
      return s(ev.message ?? ev.detail ?? '')
  }
}

const SPECS: { key: string; label: string; hint: string; type: 'number' | 'select' | 'bool' | 'text'; min?: number; max?: number; step?: number; options?: string[] }[] = [
  { key: 'autoswitch.threshold', label: 'Threshold', hint: 'Switch when the binding 5h/7d window reaches this %. 50–99.9', type: 'number', min: 50, max: 99.9, step: 1 },
  { key: 'autoswitch.intervalSeconds', label: 'Poll interval (s)', hint: 'How often the loop checks. 15–3600', type: 'number', min: 15, max: 3600, step: 5 },
  { key: 'autoswitch.cooldownSeconds', label: 'Cooldown (s)', hint: 'Minimum time between proactive switches', type: 'number', min: 0, max: 86400, step: 30 },
  { key: 'autoswitch.hysteresisPct', label: 'Hysteresis (%)', hint: 'A target must beat the active account by this much', type: 'number', min: 0, max: 50, step: 1 },
  { key: 'autoswitch.strategy', label: 'Strategy', hint: 'best = most quota left · consume-first = burn the soonest-resetting weekly window first', type: 'select', options: ['best', 'consume-first'] },
  { key: 'autoswitch.model', label: 'Model limits', hint: 'Also switch when these per-model weekly limits are hit, e.g. Fable or Fable,Opus or all', type: 'text' },
  { key: 'autoswitch.unhealthyTicks', label: 'Unhealthy ticks', hint: 'Consecutive failed polls before an account is unhealthy', type: 'number', min: 1, max: 100, step: 1 },
  { key: 'autoswitch.includeApiKeyAccounts', label: 'Include API-key accounts', hint: 'Allow rotating onto managed API-key accounts (bill per token)', type: 'bool' }
]

export function AutoPage(): React.JSX.Element {
  const { auto, autoEvents, settings, updateSettings, now } = useStore()
  const { notify } = useToast()
  const [config, setConfig] = useState<ConfigSetting[] | null>(null)
  const [cfgErr, setCfgErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState<'all' | 'important'>('important')
  const [onceEvents, setOnceEvents] = useState<AutoEvent[]>([])
  const [checking, setChecking] = useState(false)
  const allEvents = useMemo(() => [...autoEvents, ...onceEvents].sort((a, b) => a.ts.localeCompare(b.ts)), [autoEvents, onceEvents])
  const feed = useRef<HTMLDivElement>(null)

  const loadConfig = async (): Promise<void> => {
    const r = await api.getConfig()
    if (r.ok) {
      setConfig(r.value.settings)
      setCfgErr(null)
    } else setCfgErr(r.error.message)
  }
  useEffect(() => void loadConfig(), [])
  useEffect(() => {
    feed.current?.scrollTo({ top: feed.current.scrollHeight })
  }, [allEvents.length, filter])

  const shown = useMemo(() => (filter === 'all' ? allEvents : allEvents.filter((e) => e.event !== 'poll' && e.event !== 'sleep')), [allEvents, filter])
  const lastPoll = [...allEvents].reverse().find((e) => e.event === 'poll')
  const switches = allEvents.filter((e) => e.event === 'switch').length

  const toggle = async (): Promise<void> => {
    setBusy(true)
    try {
      if (auto.running) {
        await api.autoStop()
        notify('info', 'Auto-switch stopped')
      } else {
        const st = await api.autoStart({ dryRun: settings.autoDryRun })
        if (!st.running) notify('error', 'Auto-switch did not start', st.lastError)
      }
    } finally {
      setBusy(false)
    }
  }

  const save = async (key: string, value: string | null): Promise<void> => {
    const r = await api.setConfig(key, value)
    if (!r.ok) notify('error', `Could not set ${key}`, r.error.message)
    else notify('ok', value === null ? `${key} reset to default` : `${key} = ${value}`, auto.running ? 'Takes effect the next time auto-switch is started.' : undefined)
    await loadConfig()
  }

  return (
    <div className="anim-fade pb-8" data-testid="page-auto">
      <PageHeader
        title="Auto-switch"
        subtitle="Runs cswap auto --json as a child process and switches before you hit a limit."
        actions={
          <>
            <label className="flex items-center gap-2 text-[12.5px] text-fg-2">
              <Switch checked={settings.autoDryRun} disabled={auto.running} onChange={(v) => void updateSettings({ autoDryRun: v })} label="Dry run" />
              Dry run
            </label>
            <Button
              variant="default"
              loading={checking}
              disabled={auto.running}
              title="cswap auto --once: evaluate once and switch if needed"
              icon={<Zap size={14} />}
              data-testid="auto-once"
              onClick={async () => {
                setChecking(true)
                const r = await api.autoOnce({ dryRun: settings.autoDryRun })
                setChecking(false)
                if (!r.ok) return notify('error', 'Check failed', r.error.message)
                setOnceEvents((prev) => [...prev, ...r.value.events])
                const label = { switched: 'Switched account', 'no-action': 'Nothing to do — below threshold', blocked: 'Wanted to switch, but no viable target', error: 'Check reported an error', unknown: `Exit ${r.value.exitCode}` }[r.value.outcome]
                notify(r.value.outcome === 'switched' ? 'ok' : r.value.outcome === 'blocked' || r.value.outcome === 'error' ? 'error' : 'info', label, settings.autoDryRun ? 'dry run' : undefined)
              }}
            >
              Check now
            </Button>
            <Button variant={auto.running ? 'default' : 'primary'} loading={busy} onClick={() => void toggle()} icon={auto.running ? <Pause size={14} /> : <Play size={14} />} data-testid="auto-toggle">
              {auto.running ? 'Stop' : 'Start'}
            </Button>
          </>
        }
      />
      <div className="grid gap-4 px-6 lg:grid-cols-[minmax(0,1fr)_340px]">
        <div className="flex min-h-[420px] flex-col gap-4">
          <div className="grid grid-cols-3 gap-3">
            <Stat label="State" value={auto.running ? (auto.dryRun ? 'Running · dry run' : 'Running') : auto.exitCode != null && auto.exitCode !== 0 ? `Exited (${auto.exitCode})` : 'Stopped'} tone={auto.running ? 'ok' : auto.exitCode ? 'danger' : 'muted'} sub={auto.running && auto.startedAt ? `since ${clockOf(auto.startedAt)} · pid ${auto.pid}` : auto.lastError} />
            <Stat label="Last poll" value={lastPoll ? relTime(lastPoll.ts, now).replace('in ', '') : '—'} sub={lastPoll ? `threshold ${String(lastPoll.threshold)}%` : 'no polls yet'} />
            <Stat label="Switches this session" value={String(switches)} sub={`${allEvents.length} events`} />
          </div>
          <Card
            className="flex min-h-0 flex-1 flex-col"
            title="Event feed"
            actions={
              <>
                <div className="inline-flex rounded-md border border-border bg-surface-2 p-[2px] text-[11.5px]">
                  {(['important', 'all'] as const).map((f) => (
                    <button key={f} onClick={() => setFilter(f)} className={cx('rounded-[4px] px-2 py-[2px] capitalize', filter === f ? 'bg-surface text-fg shadow-card' : 'text-fg-2')}>
                      {f}
                    </button>
                  ))}
                </div>
              </>
            }
          >
            <div ref={feed} className="max-h-[52vh] min-h-[260px] overflow-y-auto font-mono text-[11.5px]" data-testid="auto-feed">
              {shown.length === 0 && (
                <div className="flex h-[260px] flex-col items-center justify-center gap-2 text-center text-fg-3">
                  <Activity size={22} />
                  <span className="font-sans text-[12.5px]">{auto.running ? 'Waiting for the first event…' : 'Start auto-switch to see polls, decisions and switches here.'}</span>
                </div>
              )}
              {shown.map((ev, i) => (
                <div key={`${ev.ts}-${i}`} className="flex items-start gap-2 border-b border-border/60 px-3 py-1.5 last:border-b-0 hover:bg-surface-2/40">
                  <span className="shrink-0 tabular-nums text-fg-3">{new Date(ev.ts).toLocaleTimeString([], { hour12: false })}</span>
                  <Chip tone={KIND_TONE[ev.event] ?? 'muted'} className="shrink-0 font-sans">
                    {ev.event}
                  </Chip>
                  <span className="selectable min-w-0 flex-1 break-words text-fg-2">{describe(ev)}</span>
                </div>
              ))}
            </div>
          </Card>
        </div>
        <Card title="cswap settings" actions={<span className="text-[11px] text-fg-3">settings.json</span>}>
          <div className="space-y-3 px-4 py-3">
            {cfgErr && <div className="text-[12px] text-danger">{cfgErr}</div>}
            {!config && !cfgErr && <div className="text-[12px] text-fg-3">Loading…</div>}
            {config &&
              SPECS.map((spec) => {
                const row = config.find((c) => c.key === spec.key)
                if (!row) return null
                return <SettingRow key={spec.key} spec={spec} row={row} onSave={save} />
              })}
            <p className="pt-1 text-[11.5px] text-fg-3">These are cswap's own settings (<span className="mono">cswap config</span>), shared with the CLI and TUI. Changes apply when auto-switch is (re)started.</p>
          </div>
        </Card>
      </div>
    </div>
  )
}

function Stat({ label, value, sub, tone = 'muted' }: { label: string; value: string; sub?: string; tone?: 'ok' | 'danger' | 'muted' }): React.JSX.Element {
  return (
    <div className="rounded-lg border border-border bg-surface px-3.5 py-2.5 shadow-card">
      <div className="text-[11px] font-medium uppercase tracking-wide text-fg-3">{label}</div>
      <div className={cx('mt-0.5 flex items-center gap-2 text-[15px] font-semibold tracking-tight', tone === 'ok' ? 'text-ok' : tone === 'danger' ? 'text-danger' : 'text-fg')}>
        {tone === 'ok' && <span className="h-[7px] w-[7px] animate-pulse rounded-full bg-ok" />}
        {value}
      </div>
      {sub && <div className="truncate text-[11.5px] text-fg-3">{sub}</div>}
    </div>
  )
}

function SettingRow({ spec, row, onSave }: { spec: (typeof SPECS)[number]; row: ConfigSetting; onSave: (k: string, v: string | null) => Promise<void> }): React.JSX.Element {
  const [val, setVal] = useState(String(row.value ?? ''))
  useEffect(() => setVal(String(row.value ?? '')), [row.value])
  const dirty = val !== String(row.value ?? '')
  const commit = (): void => {
    if (dirty) void onSave(spec.key, val)
  }
  return (
    <div className="flex items-start justify-between gap-3" data-testid={`cfg-${spec.key}`}>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 text-[12.5px] font-medium text-fg">
          {spec.label}
          {row.isSet && (
            <button className="text-[10.5px] font-normal text-fg-3 underline-offset-2 hover:underline" onClick={() => void onSave(spec.key, null)} title="Back to the default">
              reset
            </button>
          )}
        </div>
        <div className="text-[11.5px] text-fg-3">{spec.hint}</div>
      </div>
      <div className="w-[120px] shrink-0">
        {spec.type === 'bool' ? (
          <div className="flex h-8 items-center justify-end">
            <Switch checked={row.value === true || row.value === 'true'} onChange={(v) => void onSave(spec.key, v ? 'true' : 'false')} label={spec.label} />
          </div>
        ) : spec.type === 'select' ? (
          <Select value={String(row.value)} onChange={(e) => void onSave(spec.key, e.target.value)} aria-label={spec.label}>
            {spec.options!.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </Select>
        ) : (
          <Input type={spec.type === 'number' ? 'number' : 'text'} min={spec.min} max={spec.max} step={spec.step} value={val} onChange={(e) => setVal(e.target.value)} onBlur={commit} onKeyDown={(e) => e.key === 'Enter' && commit()} aria-label={spec.label} className={cx('text-right', dirty && 'border-accent')} />
        )}
      </div>
    </div>
  )
}

