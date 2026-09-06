import { Activity, FolderTree, PanelLeftClose, PanelLeftOpen, ScrollText, Settings, Users } from 'lucide-react'
import { useStore, type Page } from '../lib/store'
import { useWindowWidth } from '../lib/use-width'
import { Tooltip } from './Tooltip'
import { cx } from './ui'

const items: { id: Page; label: string; icon: React.JSX.Element }[] = [
  { id: 'accounts', label: 'Accounts', icon: <Users size={15} /> },
  { id: 'auto', label: 'Auto-switch', icon: <Activity size={15} /> },
  { id: 'mappings', label: 'Mappings', icon: <FolderTree size={15} /> },
  { id: 'log', label: 'Log', icon: <ScrollText size={15} /> },
  { id: 'settings', label: 'Settings', icon: <Settings size={15} /> }
]

export function Sidebar(): React.JSX.Element {
  const { page, setPage, binary, auto, accounts, updater, settings, updateSettings } = useStore()
  const count = accounts.payload?.accounts.length
  // Below this the labels cost more than they explain: the icons carry the meaning and
  // the room goes to the content instead. Above it, it is a choice — the button stays,
  // so a wide window can be narrowed on purpose.
  const forced = useWindowWidth() < 860
  const narrow = forced || settings.sidebarCollapsed
  return (
    <nav className={cx('flex shrink-0 flex-col gap-[2px] border-r border-border bg-surface/40 py-3 transition-[width] duration-150', narrow ? 'w-[56px] items-center px-2' : 'w-[204px] px-3')} aria-label="Main" data-compact={narrow || undefined}>
      {items.map((it) => (
        <Tooltip key={it.id} label={narrow ? it.label : ''} side="right" focusable={false} className={narrow ? '' : 'w-full'}>
          <button
            onClick={() => setPage(it.id)}
            aria-current={page === it.id ? 'page' : undefined}
            aria-label={it.label}
            data-testid={`nav-${it.id}`}
            className={cx(
              'flex h-[32px] items-center gap-2.5 rounded-md text-[13px] transition-colors duration-150 focus-visible:!shadow-[inset_0_0_0_2px_var(--accent)]',
              narrow ? 'w-[40px] justify-center px-0' : 'w-full px-2.5',
              page === it.id ? 'bg-surface-2 font-medium text-fg' : 'text-fg-2 hover:bg-surface-2/70 hover:text-fg'
            )}
          >
            <span className={cx('relative', page === it.id ? 'text-accent' : 'text-fg-3')}>
              {it.icon}
              {narrow && it.id === 'auto' && auto.running && <span className="absolute -right-1 -top-1 h-[6px] w-[6px] rounded-full bg-ok" />}
              {narrow && it.id === 'settings' && updater.status === 'downloaded' && <span className="absolute -right-1 -top-1 h-[6px] w-[6px] rounded-full bg-accent" />}
            </span>
            {!narrow && <span className="flex-1 text-left">{it.label}</span>}
            {!narrow && it.id === 'accounts' && count !== undefined && <span className="text-[11px] text-fg-3">{count}</span>}
            {!narrow && it.id === 'auto' && auto.running && <span className="h-[6px] w-[6px] rounded-full bg-ok" title="Auto-switch running" />}
            {!narrow && it.id === 'settings' && updater.status === 'downloaded' && <span className="h-[6px] w-[6px] rounded-full bg-accent" title={`Update ${updater.version} ready`} />}
          </button>
        </Tooltip>
      ))}
      <div className="flex-1" />
      {!forced && (
        <Tooltip label={narrow ? 'Expand the sidebar' : 'Collapse the sidebar'} side="right" focusable={false} className={narrow ? 'mb-1' : 'mb-1 w-full'}>
          <button
            onClick={() => void updateSettings({ sidebarCollapsed: !settings.sidebarCollapsed })}
            aria-label={narrow ? 'Expand the sidebar' : 'Collapse the sidebar'}
            aria-expanded={!narrow}
            data-testid="sidebar-toggle"
            className={cx(
              'flex h-[28px] items-center gap-2.5 rounded-md text-[12px] text-fg-3 transition-colors duration-150 hover:bg-surface-2/70 hover:text-fg focus-visible:!shadow-[inset_0_0_0_2px_var(--accent)]',
              narrow ? 'w-[40px] justify-center px-0' : 'w-full px-2.5'
            )}
          >
            {narrow ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
            {!narrow && <span className="flex-1 text-left">Collapse</span>}
          </button>
        </Tooltip>
      )}
      <Tooltip label={narrow ? (binary?.version ? `cswap ${binary.version}` : 'cswap not found') : ''} side="right" focusable={false}>
        <div className={cx('pb-1 text-[11px] text-fg-3', narrow ? 'px-0' : 'px-2.5')} data-testid="sidebar-status">
          {binary === null ? (
            narrow ? <span className="block h-[6px] w-[6px] rounded-full bg-border-strong" /> : 'detecting cswap…'
          ) : binary.version ? (
            <span className="flex items-center gap-1.5">
              <span className="h-[6px] w-[6px] rounded-full bg-ok" />
              {!narrow && `cswap ${binary.version}`}
            </span>
          ) : (
            <span className="flex items-center gap-1.5 text-danger">
              <span className="h-[6px] w-[6px] rounded-full bg-danger" />
              {!narrow && 'cswap not found'}
            </span>
          )}
        </div>
      </Tooltip>
    </nav>
  )
}
