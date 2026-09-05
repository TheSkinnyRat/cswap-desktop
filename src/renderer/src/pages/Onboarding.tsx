import { useState } from 'react'
import { Download, FolderOpen, RefreshCw, Settings, Terminal } from 'lucide-react'
import { useStore } from '../lib/store'
import { api, platform } from '../lib/api'
import { useToast } from '../lib/toast'
import { Button } from '../components/ui'

export function Onboarding(): React.JSX.Element {
  const { binary, redetect, setPage, updateSettings } = useStore()
  const { notify } = useToast()
  const [busy, setBusy] = useState<string | null>(null)
  const cmd = 'uv tool install claude-swap'
  return (
    <div className="anim-fade flex h-full items-center justify-center px-6" data-testid="onboarding">
      <div className="w-[560px] max-w-full rounded-xl border border-border bg-surface p-6 shadow-card">
        <div className="mb-1 flex items-center gap-2 text-[15px] font-semibold text-fg">
          <Terminal size={16} className="text-accent" /> cswap is not installed yet
        </div>
        <p className="text-[12.5px] text-fg-2">
          This app is a front-end for the <span className="font-medium text-fg">claude-swap</span> CLI — it does not bundle it, so one install serves the terminal, the TUI and this window. Install it with uv (recommended) or pipx, then detect again.
        </p>
        {binary?.error && binary.path && (
          <div className="mt-3 rounded-md border border-danger/30 bg-danger-soft px-3 py-2 text-[12px] text-danger">
            <span className="mono">{binary.path}</span>: {binary.error}
          </div>
        )}
        <div className="mt-4 rounded-lg border border-border bg-bg px-3 py-2.5">
          <div className="mb-1 text-[10.5px] font-medium uppercase tracking-wide text-fg-3">Terminal</div>
          <code className="mono selectable block text-fg">{cmd}</code>
          <code className="mono selectable mt-1 block text-fg-3"># or: pipx install claude-swap</code>
          <code className="mono selectable mt-1 block text-fg-3">cswap add {'  '}# after logging into Claude Code</code>
        </div>
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <Button
            variant="primary"
            icon={<Download size={14} />}
            loading={busy === 'install'}
            onClick={async () => {
              setBusy('install')
              const r = await api.installCswap()
              setBusy(null)
              if (r.ok) {
                notify('ok', 'claude-swap installed')
                await redetect()
              } else notify('error', 'Install failed', r.error.message)
            }}
            data-testid="onboarding-install"
          >
            Install with uv
          </Button>
          <Button icon={<RefreshCw size={14} />} loading={busy === 'detect'} onClick={async () => { setBusy('detect'); await redetect(); setBusy(null) }} data-testid="onboarding-detect">
            Detect again
          </Button>
          <Button
            icon={<FolderOpen size={14} />}
            onClick={async () => {
              const p = await api.pickBinary()
              if (p) await updateSettings({ cswapPath: p })
            }}
          >
            Locate cswap{platform === 'win32' ? '.exe' : ''}…
          </Button>
          <Button variant="ghost" icon={<Settings size={14} />} onClick={() => setPage('settings')} className="ml-auto">
            Settings
          </Button>
        </div>
        <p className="mt-4 text-[11.5px] text-fg-3">
          Looked in: {binary?.candidates.join(', ') || '—'} and PATH. Apps launched from a desktop shortcut can see a shorter PATH than your shell — if cswap works in a terminal but not here, point to the executable directly.
        </p>
      </div>
    </div>
  )
}
