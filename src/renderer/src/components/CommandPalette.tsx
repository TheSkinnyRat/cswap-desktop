import { useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ArrowRightLeft, CornerDownLeft } from 'lucide-react'
import { useStore } from '../lib/store'
import { api } from '../lib/api'
import { useToast } from '../lib/toast'
import { Kbd, cx } from './ui'
import { pct } from '../lib/format'

export function CommandPalette({ open, onClose }: { open: boolean; onClose: () => void }): React.JSX.Element | null {
  const { accounts, setPage } = useStore()
  const { notify } = useToast()
  const [q, setQ] = useState('')
  const [i, setI] = useState(0)
  const input = useRef<HTMLInputElement>(null)

  const rows = useMemo(() => {
    const list = accounts.payload?.accounts ?? []
    const needle = q.trim().toLowerCase()
    const acc = list
      .filter((a) => !needle || a.email.toLowerCase().includes(needle) || a.alias?.toLowerCase().includes(needle) || String(a.number) === needle || a.organizationName.toLowerCase().includes(needle))
      .map((a) => ({ kind: 'account' as const, id: `a${a.number}`, a }))
    const pages = (['accounts', 'auto', 'mappings', 'log', 'settings'] as const).filter((p) => needle && p.includes(needle)).map((p) => ({ kind: 'page' as const, id: `p${p}`, p }))
    return [...acc, ...pages]
  }, [accounts, q])

  useEffect(() => {
    if (open) {
      setQ('')
      setI(0)
      setTimeout(() => input.current?.focus(), 0)
    }
  }, [open])
  useEffect(() => setI(0), [q])

  const run = async (idx: number): Promise<void> => {
    const r = rows[idx]
    if (!r) return
    onClose()
    if (r.kind === 'page') return setPage(r.p)
    if (r.a.active) return
    const res = await api.switchTo(String(r.a.number))
    if (res.ok) notify(res.value.switched ? 'ok' : 'info', res.value.message, res.value.warnings.join('\n') || undefined)
    else notify('error', 'Switch failed', res.error.message)
  }

  if (!open) return null
  return createPortal(
    <div className="fixed inset-0 z-[85] flex items-start justify-center bg-black/25 pt-[14vh]" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div role="dialog" aria-label="Switch account" className="anim-pop w-[520px] max-w-[calc(100vw-32px)] overflow-hidden rounded-xl border border-border bg-surface shadow-pop">
        <input
          ref={input}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') setI((x) => Math.min(rows.length - 1, x + 1))
            else if (e.key === 'ArrowUp') setI((x) => Math.max(0, x - 1))
            else if (e.key === 'Enter') void run(i)
            else return
            e.preventDefault()
          }}
          placeholder="Switch to account… (type an email, alias or slot)"
          className="h-11 w-full border-b border-border bg-transparent px-4 text-[14px] text-fg placeholder:text-fg-3"
          data-testid="palette-input"
        />
        <ul className="max-h-[340px] overflow-y-auto p-1.5" role="listbox">
          {rows.length === 0 && <li className="px-3 py-6 text-center text-[12.5px] text-fg-3">No matches</li>}
          {rows.map((r, idx) => (
            <li
              key={r.id}
              role="option"
              aria-selected={idx === i}
              onMouseEnter={() => setI(idx)}
              onClick={() => void run(idx)}
              className={cx('flex cursor-default items-center gap-3 rounded-md px-2.5 py-2 text-[13px]', idx === i ? 'bg-surface-2' : '')}
            >
              {r.kind === 'account' ? (
                <>
                  <span className="mono w-5 text-right text-fg-3">{r.a.number}</span>
                  <span className={cx('h-[6px] w-[6px] rounded-full', r.a.active ? 'bg-accent' : 'bg-border-strong')} />
                  <span className="min-w-0 flex-1 truncate text-fg">
                    {r.a.alias && <span className="mr-1.5 font-medium">{r.a.alias}</span>}
                    <span className={r.a.alias ? 'text-fg-2' : ''}>{r.a.email}</span>
                  </span>
                  {r.a.usage?.fiveHour && <span className="text-[11.5px] text-fg-3">5h {pct(r.a.usage.fiveHour.pct)} · 7d {pct(r.a.usage.sevenDay?.pct)}</span>}
                  {r.a.active ? <span className="text-[11px] text-accent">active</span> : <ArrowRightLeft size={13} className="text-fg-3" />}
                </>
              ) : (
                <>
                  <span className="mono w-5 text-right text-fg-3">→</span>
                  <span className="flex-1 capitalize text-fg">Go to {r.p}</span>
                </>
              )}
            </li>
          ))}
        </ul>
        <div className="flex items-center gap-3 border-t border-border px-3 py-1.5 text-[11px] text-fg-3">
          <span className="flex items-center gap-1">
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd> navigate
          </span>
          <span className="flex items-center gap-1">
            <Kbd>
              <CornerDownLeft size={10} />
            </Kbd>{' '}
            switch
          </span>
          <span className="flex items-center gap-1">
            <Kbd>esc</Kbd> close
          </span>
        </div>
      </div>
    </div>,
    document.body
  )
}
