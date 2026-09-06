import { RefreshCw, Search } from 'lucide-react'
import { useStore } from '../lib/store'
import { platform, isElectron } from '../lib/api'
import { Kbd, cx } from './ui'
import { ageSeconds, maskEmail } from '../lib/format'

export function TitleBar({ onPalette }: { onPalette: () => void }): React.JSX.Element {
  const { accounts, refresh, now, settings } = useStore()
  const active = accounts.payload?.accounts.find((a) => a.active)
  const mac = platform === 'darwin'
  const fetchedAge = accounts.fetchedAt ? ageSeconds((now - new Date(accounts.fetchedAt).getTime()) / 1000) : ''
  return (
    <header className={cx('drag flex h-[38px] shrink-0 items-center gap-3 border-b border-border bg-surface/70 pr-[140px]', mac ? 'pl-[76px]' : 'pl-3')} data-testid="titlebar">
      <div className="flex items-center gap-2 text-[12.5px] font-semibold tracking-tight text-fg">
        <span className="inline-block h-[14px] w-[14px] rounded-[4px] bg-accent" />
        cswap
      </div>
      <div className="flex-1" />
      <button
        onClick={onPalette}
        className="no-drag inline-flex h-[26px] items-center gap-2 rounded-md border border-border bg-bg px-2 text-[12px] text-fg-3 hover:border-border-strong hover:text-fg-2"
        data-testid="palette-trigger"
      >
        <Search size={13} />
        <span className="hidden sm:inline">Switch account…</span>
        <Kbd>{mac ? '⌘' : 'Ctrl'} K</Kbd>
      </button>
      {active && (
        <div className="no-drag hidden items-center gap-2 rounded-md bg-accent-soft px-2 py-[3px] text-[12px] text-accent md:flex" data-testid="active-pill">
          <span className="h-[6px] w-[6px] rounded-full bg-accent" />
          <span className="font-medium">{active.alias || maskEmail(active.email, settings.maskEmails)}</span>
        </div>
      )}
      <button
        onClick={() => void refresh()}
        title={fetchedAge ? `Refresh usage · updated ${fetchedAge}` : 'Refresh usage'}
        aria-label="Refresh usage"
        className="no-drag inline-flex h-[26px] items-center gap-1.5 rounded-md px-1.5 text-fg-3 hover:bg-surface-2 hover:text-fg"
        data-testid="refresh"
      >
        <RefreshCw size={13} className={accounts.refreshing ? 'spin' : ''} />
        {fetchedAge && <span className="text-[11px]">{fetchedAge}</span>}
      </button>
      {!isElectron && <span className="no-drag text-[10px] uppercase tracking-wide text-fg-3">browser mock</span>}
    </header>
  )
}
