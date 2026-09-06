import { Activity, FolderTree, ScrollText, Settings, Users } from 'lucide-react'
import { useStore, type Page } from '../lib/store'
import { cx } from './ui'

const items: { id: Page; label: string; icon: React.JSX.Element }[] = [
  { id: 'accounts', label: 'Accounts', icon: <Users size={15} /> },
  { id: 'auto', label: 'Auto-switch', icon: <Activity size={15} /> },
  { id: 'mappings', label: 'Mappings', icon: <FolderTree size={15} /> },
  { id: 'log', label: 'Log', icon: <ScrollText size={15} /> },
  { id: 'settings', label: 'Settings', icon: <Settings size={15} /> }
]

export function Sidebar(): React.JSX.Element {
  const { page, setPage, binary, auto, accounts, updater } = useStore()
  const count = accounts.payload?.accounts.length
  return (
    <nav className="flex w-[204px] shrink-0 flex-col gap-[2px] border-r border-border bg-surface/40 px-3 py-3" aria-label="Main">
      {items.map((it) => (
        <button
          key={it.id}
          onClick={() => setPage(it.id)}
          aria-current={page === it.id ? 'page' : undefined}
          data-testid={`nav-${it.id}`}
          className={cx('flex h-[32px] items-center gap-2.5 rounded-md px-2.5 text-[13px] transition-colors duration-150 focus-visible:!shadow-[inset_0_0_0_2px_var(--accent)]', page === it.id ? 'bg-surface-2 font-medium text-fg' : 'text-fg-2 hover:bg-surface-2/70 hover:text-fg')}
        >
          <span className={page === it.id ? 'text-accent' : 'text-fg-3'}>{it.icon}</span>
          <span className="flex-1 text-left">{it.label}</span>
          {it.id === 'accounts' && count !== undefined && <span className="text-[11px] text-fg-3">{count}</span>}
          {it.id === 'auto' && auto.running && <span className="h-[6px] w-[6px] rounded-full bg-ok" title="Auto-switch running" />}
          {it.id === 'settings' && updater.status === 'downloaded' && <span className="h-[6px] w-[6px] rounded-full bg-accent" title={`Update ${updater.version} ready`} />}
        </button>
      ))}
      <div className="flex-1" />
      <div className="px-2.5 pb-1 text-[11px] text-fg-3" data-testid="sidebar-status">
        {binary === null ? (
          'detecting cswap…'
        ) : binary.version ? (
          <span className="flex items-center gap-1.5">
            <span className="h-[6px] w-[6px] rounded-full bg-ok" />
            cswap {binary.version}
          </span>
        ) : (
          <span className="flex items-center gap-1.5 text-danger">
            <span className="h-[6px] w-[6px] rounded-full bg-danger" />
            cswap not found
          </span>
        )}
      </div>
    </nav>
  )
}
