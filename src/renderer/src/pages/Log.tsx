import { useState } from 'react'
import { ChevronRight, ScrollText, Trash2 } from 'lucide-react'
import { useStore } from '../lib/store'
import { api } from '../lib/api'
import { Button, Chip, Empty, cx } from '../components/ui'
import { PageHeader } from '../components/PageHeader'

export function LogPage(): React.JSX.Element {
  const { log } = useStore()
  const [open, setOpen] = useState<number | null>(null)
  const [, force] = useState(0)
  const rows = [...log].reverse()
  return (
    <div className="anim-fade pb-8" data-testid="page-log">
      <PageHeader
        title="Command log"
        subtitle="Every cswap invocation this app made — arguments, exit code, duration, output. Tokens never appear here (they travel over stdin)."
        actions={
          <Button
            variant="ghost"
            icon={<Trash2 size={14} />}
            disabled={!log.length}
            onClick={async () => {
              await api.clearCommandLog()
              log.length = 0
              force((x) => x + 1)
            }}
          >
            Clear
          </Button>
        }
      />
      <div className="px-6">
        {rows.length === 0 && <Empty icon={<ScrollText size={28} />} title="Nothing yet" body="Commands will show up here as the app talks to cswap." />}
        {rows.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-border bg-surface shadow-card">
            {rows.map((e) => (
              <div key={e.id} className="border-b border-border last:border-b-0" data-testid="log-row">
                <button onClick={() => setOpen(open === e.id ? null : e.id)} className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-surface-2/50">
                  <ChevronRight size={13} className={cx('shrink-0 text-fg-3 transition-transform duration-150', open === e.id && 'rotate-90')} />
                  <span className="shrink-0 tabular-nums text-[11.5px] text-fg-3">{new Date(e.at).toLocaleTimeString([], { hour12: false })}</span>
                  <span className="mono min-w-0 flex-1 truncate text-fg">cswap {e.args.join(' ')}</span>
                  <span className="shrink-0 text-[11.5px] text-fg-3">{e.durationMs} ms</span>
                  <Chip tone={e.ok ? 'ok' : 'danger'}>{e.error ? 'spawn error' : `exit ${e.exitCode ?? '—'}`}</Chip>
                </button>
                {open === e.id && (
                  <div className="selectable grid gap-2 border-t border-border/60 bg-bg/40 px-3 py-2 font-mono text-[11.5px]">
                    {e.error && <pre className="whitespace-pre-wrap text-danger">{e.error}</pre>}
                    {e.stdout?.trim() && (
                      <div>
                        <div className="mb-0.5 font-sans text-[10.5px] uppercase tracking-wide text-fg-3">stdout</div>
                        <pre className="max-h-[300px] overflow-auto whitespace-pre-wrap text-fg-2">{e.stdout.trim()}</pre>
                      </div>
                    )}
                    {e.stderr?.trim() && (
                      <div>
                        <div className="mb-0.5 font-sans text-[10.5px] uppercase tracking-wide text-fg-3">stderr</div>
                        <pre className="max-h-[300px] overflow-auto whitespace-pre-wrap text-warn">{e.stderr.trim()}</pre>
                      </div>
                    )}
                    {!e.stdout?.trim() && !e.stderr?.trim() && !e.error && <span className="text-fg-3">(no output)</span>}
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
