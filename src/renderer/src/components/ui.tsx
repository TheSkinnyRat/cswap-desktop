import { forwardRef, useEffect, useId, useLayoutEffect, useRef, useState, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from 'react'
import { createPortal } from 'react-dom'
import { useGrow } from '../lib/use-grow'
import { Check, ChevronDown, Loader2, X } from 'lucide-react'

export function cx(...parts: (string | false | null | undefined)[]): string {
  return parts.filter(Boolean).join(' ')
}

// ---- Button ------------------------------------------------------------------
type Variant = 'primary' | 'default' | 'ghost' | 'danger' | 'subtle'
type Size = 'sm' | 'md'
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant
  size?: Size
  loading?: boolean
  icon?: ReactNode
}
const variants: Record<Variant, string> = {
  primary: 'bg-accent text-accent-fg hover:bg-accent-hover border-transparent shadow-[inset_0_1px_0_rgba(255,255,255,0.12)]',
  default: 'bg-surface text-fg border-border hover:bg-surface-2 hover:border-border-strong',
  subtle: 'bg-surface-2 text-fg border-transparent hover:bg-surface-3',
  ghost: 'bg-transparent text-fg-2 border-transparent hover:bg-surface-2 hover:text-fg',
  danger: 'bg-danger text-white border-transparent hover:brightness-110'
}
const sizes: Record<Size, string> = { sm: 'h-7 px-2.5 text-[12px] gap-1.5', md: 'h-8 px-3 text-[13px] gap-2' }
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button({ variant = 'default', size = 'md', loading, icon, className, children, disabled, ...rest }, ref) {
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cx('inline-flex select-none items-center justify-center whitespace-nowrap rounded-md border font-medium transition-colors duration-150 disabled:cursor-not-allowed disabled:opacity-50', variants[variant], sizes[size], className)}
      {...rest}
    >
      {loading ? <Loader2 size={14} className="spin" /> : icon}
      {children}
    </button>
  )
})

export function IconButton({ label, className, size = 'md', ...rest }: ButtonProps & { label: string }): React.JSX.Element {
  return <Button aria-label={label} title={label} variant="ghost" size={size} className={cx('!px-0', size === 'sm' ? 'w-7' : 'w-8', className)} {...rest} />
}

// ---- Chip ----------------------------------------------------------------------
export function Chip({ tone = 'muted', children, className, title }: { tone?: 'ok' | 'warn' | 'danger' | 'muted' | 'accent'; children: ReactNode; className?: string; title?: string }): React.JSX.Element {
  const t = {
    ok: 'bg-ok-soft text-ok',
    warn: 'bg-warn-soft text-warn',
    danger: 'bg-danger-soft text-danger',
    muted: 'bg-surface-2 text-fg-2',
    accent: 'bg-accent-soft text-accent'
  }[tone]
  return (
    <span title={title} className={cx('inline-flex h-[18px] items-center whitespace-nowrap rounded px-1.5 text-[11px] font-medium leading-none', t, className)}>
      {children}
    </span>
  )
}

// ---- Inputs --------------------------------------------------------------------
export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input({ className, ...rest }, ref) {
  return <input ref={ref} className={cx('h-8 w-full rounded-md border border-border bg-surface px-2.5 text-[13px] text-fg placeholder:text-fg-3 hover:border-border-strong focus:border-accent', className)} {...rest} />
})

export function Select({ className, children, ...rest }: SelectHTMLAttributes<HTMLSelectElement>): React.JSX.Element {
  return (
    <div className={cx('relative', className)}>
      <select className="h-8 w-full appearance-none rounded-md border border-border bg-surface pl-2.5 pr-7 text-[13px] text-fg hover:border-border-strong focus:border-accent" {...rest}>
        {children}
      </select>
      <ChevronDown size={14} className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-fg-3" />
    </div>
  )
}

export function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (v: boolean) => void; label?: string; disabled?: boolean }): React.JSX.Element {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx('relative inline-flex h-[18px] w-[30px] shrink-0 items-center rounded-full border transition-colors duration-150 disabled:opacity-50', checked ? 'border-accent bg-accent' : 'border-border-strong bg-surface-3')}
    >
      <span className={cx('absolute top-[2px] h-[12px] w-[12px] rounded-full bg-white shadow transition-[left] duration-150', checked ? 'left-[14px]' : 'left-[2px]')} />
    </button>
  )
}

export function Segmented<T extends string>({ value, onChange, options, ariaLabel }: { value: T; onChange: (v: T) => void; options: { value: T; label: ReactNode }[]; ariaLabel?: string }): React.JSX.Element {
  return (
    <div role="radiogroup" aria-label={ariaLabel} className="inline-flex h-8 items-center rounded-md border border-border bg-surface-2 p-[2px]">
      {options.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={value === o.value}
          onClick={() => onChange(o.value)}
          className={cx('inline-flex h-full items-center gap-1.5 rounded-[5px] px-2.5 text-[12px] font-medium transition-colors duration-150', value === o.value ? 'bg-surface text-fg shadow-card' : 'text-fg-2 hover:text-fg')}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}

export function Field({ label, hint, children, inline }: { label: string; hint?: ReactNode; children: ReactNode; inline?: boolean }): React.JSX.Element {
  return (
    <label className={cx('flex gap-1', inline ? 'flex-row items-center justify-between gap-4' : 'flex-col')}>
      <span className="min-w-0">
        <span className="block text-[13px] font-medium text-fg">{label}</span>
        {hint && <span className="block text-[12px] text-fg-3">{hint}</span>}
      </span>
      {children}
    </label>
  )
}

// ---- Usage bar -------------------------------------------------------------------
export function Bar({ pct, tone, className }: { pct: number | undefined; tone: 'ok' | 'warn' | 'danger' | 'muted'; className?: string }): React.JSX.Element {
  const fill = { ok: 'bg-accent', warn: 'bg-warn', danger: 'bg-danger', muted: 'bg-border-strong' }[tone]
  // Grown into rather than drawn at, like the ring: a transition needs a change.
  const w = useGrow(pct === undefined ? 0 : Math.max(0, Math.min(100, pct)))
  return (
    <div className={cx('h-[5px] w-full overflow-hidden rounded-full bg-surface-3', className)} role="progressbar" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
      <div className={cx('h-full rounded-full transition-[width] duration-500 ease-out motion-reduce:transition-none', fill)} style={{ width: `${w}%` }} />
    </div>
  )
}

// ---- Dropdown menu -----------------------------------------------------------------
export interface MenuItem {
  label: ReactNode
  onSelect?: () => void
  danger?: boolean
  disabled?: boolean
  icon?: ReactNode
  separator?: boolean
  checked?: boolean
  hint?: string
}
export function Menu({ trigger, items, align = 'end', width = 220 }: { trigger: (open: boolean) => ReactNode; items: MenuItem[]; align?: 'start' | 'end'; width?: number }): React.JSX.Element {
  // `width` is a minimum; the menu grows to fit its longest label so nothing is cut.
  const [open, setOpen] = useState(false)
  const btn = useRef<HTMLDivElement>(null)
  const pop = useRef<HTMLDivElement>(null)
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null)
  useLayoutEffect(() => {
    if (!open || !btn.current) return
    const r = btn.current.getBoundingClientRect()
    const h = pop.current?.offsetHeight ?? 0
    const w = Math.max(width, pop.current?.offsetWidth ?? 0)
    const top = r.bottom + 4 + h > window.innerHeight - 8 ? Math.max(8, r.top - 4 - h) : r.bottom + 4
    const left = align === 'end' ? Math.max(8, r.right - w) : Math.min(r.left, window.innerWidth - w - 8)
    setPos({ top, left })
  }, [open, align, width])
  useEffect(() => {
    if (!open) return
    const onDoc = (e: MouseEvent): void => {
      if (!pop.current?.contains(e.target as Node) && !btn.current?.contains(e.target as Node)) setOpen(false)
    }
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDoc)
      document.removeEventListener('keydown', onKey)
    }
  }, [open])
  return (
    <>
      <div ref={btn} className="inline-flex" onClick={() => setOpen((o) => !o)}>
        {trigger(open)}
      </div>
      {open &&
        createPortal(
          <div ref={pop} role="menu" style={{ top: pos?.top ?? -9999, left: pos?.left ?? -9999, minWidth: width, maxWidth: 'calc(100vw - 16px)' }} className="anim-pop fixed z-[90] w-max rounded-lg border border-border bg-surface p-1 shadow-pop">
            {items.map((it, i) =>
              it.separator ? (
                <div key={i} className="my-1 h-px bg-border" />
              ) : (
                <button
                  key={i}
                  role="menuitem"
                  disabled={it.disabled}
                  onClick={() => {
                    setOpen(false)
                    it.onSelect?.()
                  }}
                  className={cx('flex w-full items-center gap-3 whitespace-nowrap rounded-md px-2 py-1.5 text-left text-[13px] disabled:opacity-40', it.danger ? 'text-danger hover:bg-danger-soft' : 'text-fg hover:bg-surface-2')}
                >
                  {it.icon && <span className="shrink-0 text-fg-3">{it.icon}</span>}
                  <span className="flex-1">{it.label}</span>
                  {it.hint && <span className="shrink-0 pl-2 text-[11px] text-fg-3">{it.hint}</span>}
                  {it.checked && <Check size={14} className="text-accent" />}
                </button>
              )
            )}
          </div>,
          document.body
        )}
    </>
  )
}

// ---- Dialog --------------------------------------------------------------------------
export function Dialog({ open, onClose, title, children, footer, width = 440 }: { open: boolean; onClose: () => void; title: ReactNode; children: ReactNode; footer?: ReactNode; width?: number }): React.JSX.Element | null {
  const id = useId()
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, onClose])
  if (!open) return null
  return createPortal(
    <div className="fixed inset-0 z-[80] flex items-start justify-center bg-black/30 pt-[12vh] backdrop-blur-[1px]" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div role="dialog" aria-modal="true" aria-labelledby={id} style={{ width }} className="anim-pop max-h-[76vh] max-w-[calc(100vw-32px)] overflow-hidden rounded-xl border border-border bg-surface shadow-pop">
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 id={id} className="text-[14px] font-semibold text-fg">
            {title}
          </h2>
          <IconButton label="Close" size="sm" onClick={onClose} icon={<X size={14} />} />
        </div>
        <div className="max-h-[calc(76vh-110px)] overflow-y-auto px-4 py-4">{children}</div>
        {footer && <div className="flex items-center justify-end gap-2 border-t border-border bg-bg/60 px-4 py-3">{footer}</div>}
      </div>
    </div>,
    document.body
  )
}

export function Empty({ icon, title, body, action }: { icon?: ReactNode; title: string; body?: ReactNode; action?: ReactNode }): React.JSX.Element {
  return (
    <div className="anim-fade flex flex-col items-center justify-center rounded-lg border border-dashed border-border px-6 py-14 text-center">
      {icon && <div className="mb-3 text-fg-3">{icon}</div>}
      <div className="text-[14px] font-medium text-fg">{title}</div>
      {body && <div className="mt-1 max-w-[420px] text-[12.5px] text-fg-2">{body}</div>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  )
}

export function Kbd({ children }: { children: ReactNode }): React.JSX.Element {
  return <kbd className="mono inline-flex h-[18px] items-center rounded border border-border bg-surface-2 px-1 text-[10.5px] text-fg-2">{children}</kbd>
}

export function Card({ children, className, title, actions }: { children: ReactNode; className?: string; title?: ReactNode; actions?: ReactNode }): React.JSX.Element {
  return (
    <section className={cx('rounded-lg border border-border bg-surface shadow-card', className)}>
      {(title || actions) && (
        <header className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
          <h3 className="text-[13px] font-semibold text-fg">{title}</h3>
          <div className="flex items-center gap-2">{actions}</div>
        </header>
      )}
      <div>{children}</div>
    </section>
  )
}
