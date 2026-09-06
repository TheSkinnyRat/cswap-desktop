import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { cx } from './ui'

const DELAY_MS = 120
const GAP = 6
const GAP_SIDE = 14

/**
 * The app's own tooltip instead of the browser's `title`: it answers on hover *and*
 * on focus, and it can be read on a dark background. Rendered through a portal and
 * anchored by its bottom edge, so a row inside the table's horizontal scroller cannot
 * clip it and its own height never has to be measured first.
 */
export function Tooltip({ label, focusable = true, className = '', side = 'top', children }: { label: string; focusable?: boolean; className?: string; side?: 'top' | 'right'; children: ReactNode }): React.JSX.Element {
  const [open, setOpen] = useState(false)
  const [spot, setSpot] = useState<{ left: number; bottom?: number; top?: number } | null>(null)
  const host = useRef<HTMLSpanElement>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const id = useId()

  const cancel = (): void => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = null
  }
  const place = useCallback(() => {
    const el = host.current
    if (!el) return
    const r = el.getBoundingClientRect()
    // Above the trigger, anchored by its bottom — or beside it, anchored by its left.
    // Beside is what a collapsed sidebar needs: above, the topmost item's card would be
    // cut off by the top of the window.
    setSpot(side === 'right' ? { left: r.right + GAP_SIDE, top: r.top + r.height / 2 } : { left: r.left + r.width / 2, bottom: window.innerHeight - r.top + GAP })
  }, [side])
  const show = useCallback(() => {
    cancel()
    place()
    timer.current = setTimeout(() => setOpen(true), DELAY_MS)
  }, [place])
  const hide = useCallback(() => {
    cancel()
    setOpen(false)
  }, [])

  useEffect(() => {
    if (!open) return
    const onMove = (): void => place()
    window.addEventListener('scroll', onMove, true)
    window.addEventListener('resize', onMove)
    return () => {
      window.removeEventListener('scroll', onMove, true)
      window.removeEventListener('resize', onMove)
    }
  }, [open, place])
  useEffect(() => cancel, [])

  if (!label) return <>{children}</>
  return (
    <span
      ref={host}
      className={`inline-flex ${className}`}
      tabIndex={focusable ? 0 : undefined}
      aria-describedby={open ? id : undefined}
      onMouseEnter={show}
      onMouseLeave={hide}
      onFocus={show}
      onBlur={hide}
    >
      {children}
      {open &&
        spot &&
        createPortal(
          <span
            id={id}
            role="tooltip"
            style={side === 'right' ? { left: spot.left, top: spot.top } : { left: spot.left, bottom: spot.bottom }}
            className={cx(
              'anim-fade pointer-events-none fixed z-[95] whitespace-pre rounded-md border border-border bg-surface px-2 py-1 text-[11.5px] text-fg shadow-pop',
              side === 'right' ? '-translate-y-1/2' : '-translate-x-1/2'
            )}
          >
            {label}
          </span>,
          document.body
        )}
    </span>
  )
}
