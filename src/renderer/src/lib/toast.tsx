import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from 'react'
import { CheckCircle2, CircleAlert, Info, X } from 'lucide-react'

export type ToastKind = 'ok' | 'error' | 'info'
interface Toast {
  id: number
  kind: ToastKind
  title: string
  detail?: string
}
interface ToastApi {
  notify: (kind: ToastKind, title: string, detail?: string) => void
}
const Ctx = createContext<ToastApi | null>(null)

export function ToastProvider({ children }: { children: ReactNode }): React.JSX.Element {
  const [toasts, setToasts] = useState<Toast[]>([])
  const seq = useRef(0)
  const dismiss = useCallback((id: number) => setToasts((t) => t.filter((x) => x.id !== id)), [])
  const notify = useCallback(
    (kind: ToastKind, title: string, detail?: string) => {
      const id = ++seq.current
      setToasts((t) => [...t, { id, kind, title, detail }])
      if (kind !== 'error') setTimeout(() => dismiss(id), 4500)
    },
    [dismiss]
  )
  const value = useMemo(() => ({ notify }), [notify])
  return (
    <Ctx.Provider value={value}>
      {children}
      <div className="pointer-events-none fixed bottom-3 right-3 z-[100] flex w-[360px] max-w-[calc(100vw-24px)] flex-col-reverse gap-2" aria-live="polite">
        {toasts.map((t) => (
          <div
            key={t.id}
            role="status"
            className="anim-pop pointer-events-auto flex items-start gap-2.5 rounded-lg border border-border bg-surface px-3 py-2.5 shadow-pop"
          >
            {t.kind === 'ok' ? <CheckCircle2 size={16} className="mt-0.5 shrink-0 text-ok" /> : t.kind === 'error' ? <CircleAlert size={16} className="mt-0.5 shrink-0 text-danger" /> : <Info size={16} className="mt-0.5 shrink-0 text-accent" />}
            <div className="min-w-0 flex-1">
              <div className="font-medium text-fg">{t.title}</div>
              {t.detail && <div className="selectable mt-0.5 break-words text-[12px] text-fg-2">{t.detail}</div>}
            </div>
            <button aria-label="Dismiss" onClick={() => dismiss(t.id)} className="rounded p-0.5 text-fg-3 hover:bg-surface-2 hover:text-fg">
              <X size={14} />
            </button>
          </div>
        ))}
      </div>
    </Ctx.Provider>
  )
}

export function useToast(): ToastApi {
  const c = useContext(Ctx)
  if (!c) throw new Error('ToastProvider missing')
  return c
}
