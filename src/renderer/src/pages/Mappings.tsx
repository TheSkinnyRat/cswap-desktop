import { useEffect, useState } from 'react'
import { FolderOpen, FolderTree, Plus, Trash2 } from 'lucide-react'
import type { Mapping } from '@shared/types'
import { useStore } from '../lib/store'
import { api } from '../lib/api'
import { useToast } from '../lib/toast'
import { Button, Chip, Dialog, Empty, Field, Input, Select } from '../components/ui'
import { PageHeader } from '../components/PageHeader'

export function MappingsPage(): React.JSX.Element {
  const { accounts } = useStore()
  const { notify } = useToast()
  const [rows, setRows] = useState<Mapping[] | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const list = accounts.payload?.accounts ?? []

  const load = async (): Promise<void> => {
    const r = await api.listMappings()
    if (r.ok) {
      setRows(r.value)
      setErr(null)
    } else setErr(r.error.message)
  }
  useEffect(() => void load(), [accounts.fetchedAt])

  const remove = async (m: Mapping): Promise<void> => {
    const r = await api.unmapDirectory(m.path)
    if (r.ok) notify('ok', `Unmapped ${m.path}`)
    else notify('error', 'Unmap failed', r.error.message)
    await load()
  }

  return (
    <div className="anim-fade pb-8" data-testid="page-mappings">
      <PageHeader
        title="Directory mappings"
        subtitle={
          <>
            <span className="mono">cswap run</span> with no account launches the account mapped to the current directory — session mode, this terminal only.
          </>
        }
        actions={
          <Button variant="primary" icon={<Plus size={14} />} onClick={() => setAdding(true)} disabled={!list.length} data-testid="map-add">
            Map a directory
          </Button>
        }
      />
      <div className="px-6">
        {err && <div className="mb-3 rounded-lg border border-danger/30 bg-danger-soft px-3 py-2 text-[12.5px] text-danger">{err}</div>}
        {rows && rows.length === 0 && (
          <Empty icon={<FolderTree size={28} />} title="No directory mappings" body="Map a project folder to an account, then `cswap run` inside it picks that account automatically." action={<Button onClick={() => setAdding(true)} disabled={!list.length}>Map a directory</Button>} />
        )}
        {rows && rows.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-border bg-surface shadow-card">
            {rows.map((m) => (
              <div key={m.path} className="flex items-center gap-3 border-b border-border px-3 py-2.5 last:border-b-0 hover:bg-surface-2/50" data-testid="mapping-row">
                <FolderOpen size={15} className="shrink-0 text-fg-3" />
                <div className="min-w-0 flex-1">
                  <div className="mono truncate text-fg" title={m.path}>
                    {m.path}
                  </div>
                  <div className="mt-0.5 flex items-center gap-1.5 text-[11.5px] text-fg-3">
                    → {m.account ? <span className="text-fg-2">Account-{m.account.number} · {m.account.alias || m.account.email}</span> : <span>{m.email}</span>}
                    {!m.account && <Chip tone="warn">account removed</Chip>}
                  </div>
                </div>
                <Button size="sm" variant="ghost" icon={<Trash2 size={13} />} onClick={() => void remove(m)} aria-label={`Unmap ${m.path}`} data-testid="unmap">
                  Unmap
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
      {adding && (
        <AddMappingDialog
          onClose={() => setAdding(false)}
          onDone={() => {
            setAdding(false)
            void load()
          }}
        />
      )}
    </div>
  )
}

function AddMappingDialog({ onClose, onDone }: { onClose: () => void; onDone: () => void }): React.JSX.Element {
  const { accounts } = useStore()
  const { notify } = useToast()
  const list = accounts.payload?.accounts ?? []
  const [target, setTarget] = useState(String(list.find((a) => a.active)?.number ?? list[0]?.number ?? ''))
  const [path, setPath] = useState('')
  const [busy, setBusy] = useState(false)
  return (
    <Dialog
      open
      onClose={onClose}
      title="Map a directory to an account"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={busy}
            disabled={!path.trim() || !target}
            data-testid="map-submit"
            onClick={async () => {
              setBusy(true)
              const r = await api.mapDirectory(target, path.trim())
              setBusy(false)
              if (r.ok) {
                notify('ok', r.value.stdout.trim().split('\n')[0] || 'Mapped')
                onDone()
              } else notify('error', 'Map failed', r.error.message)
            }}
          >
            Map
          </Button>
        </>
      }
    >
      <Field label="Account">
        <Select value={target} onChange={(e) => setTarget(e.target.value)} data-testid="map-account">
          {list.map((a) => (
            <option key={a.number} value={String(a.number)}>
              {a.number}. {a.alias ? `${a.alias} — ` : ''}
              {a.email}
            </option>
          ))}
        </Select>
      </Field>
      <div className="mt-3">
        <Field label="Directory">
          <div className="flex gap-2">
            <Input value={path} onChange={(e) => setPath(e.target.value)} placeholder="/path/to/project" className="mono" data-testid="map-path" />
            <Button
              onClick={async () => {
                const p = await api.pickDirectory()
                if (p) setPath(p)
              }}
              icon={<FolderOpen size={14} />}
            >
              Browse
            </Button>
          </div>
        </Field>
      </div>
    </Dialog>
  )
}
