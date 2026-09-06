import { Tooltip } from './Tooltip'
import { cx } from './ui'
import { pct as fmtPct } from '../lib/format'
import { useGrow } from '../lib/use-grow'

const SIZE = 30
const STROKE = 3.5
// Room is left between the stroke and the edge of the box so the pace notch can sit
// OUTSIDE the ring: drawn across the track, a full arc simply covers it.
const R = (SIZE - STROKE) / 2 - 2.5
const C = 2 * Math.PI * R

// Same thresholds as the bars, so one glance means the same thing in both views.
const ringTone = (p: number): string => (p >= 100 ? 'stroke-danger' : p >= 80 ? 'stroke-warn' : 'stroke-accent')

export function UsageRing({ pct, label, tooltip, sub, pace }: { pct: number | undefined; label?: string; tooltip: string; sub?: string; pace?: number }): React.JSX.Element {
  const value = pct === undefined ? 0 : Math.max(0, Math.min(pct, 100))
  const filled = useGrow(value)
  // Where an evenly-spread week would be, as a notch on the track.
  const mark = pace === undefined ? null : Math.max(0, Math.min(100, pace))
  return (
    <Tooltip label={tooltip} focusable={false} className="min-w-[92px] items-center gap-2 rounded-md px-1 py-0.5 transition-colors duration-150 hover:bg-surface-2">
      <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} className="shrink-0 -rotate-90" data-testid="usage-ring" aria-hidden>
        <circle cx={SIZE / 2} cy={SIZE / 2} r={R} fill="none" strokeWidth={STROKE} className="stroke-border-strong opacity-40" />
        {mark !== null && (
          <line
            x1={SIZE / 2}
            y1={0.6}
            x2={SIZE / 2}
            y2={SIZE / 2 - R - STROKE / 2 - 0.4}
            strokeWidth="2"
            strokeLinecap="round"
            className="stroke-fg-2"
            // +90: the arc starts at the local 3 o'clock (where a dasharray begins) while
            // this line is drawn at the local 12, and the whole svg is turned -90 so the
            // fill reads from the top. Without it every notch sat a quarter turn early.
            transform={`rotate(${(mark / 100) * 360 + 90} ${SIZE / 2} ${SIZE / 2})`}
          />
        )}
        <circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={R}
          fill="none"
          strokeWidth={STROKE}
          strokeLinecap="round"
          // stroke-dasharray is the one length that describes how full a ring is, so it
          // is the one that moves. A zero-length dash with a round cap still paints a
          // dot, which reads as "a little used" when the answer is none — hidden at 0.
          strokeDasharray={`${(C * filled) / 100} ${C}`}
          className={cx('transition-[stroke-dasharray,stroke] duration-500 ease-out motion-reduce:transition-none', ringTone(value), filled === 0 && 'opacity-0')}
        />
      </svg>
      <span className="min-w-0">
        <span className="block truncate text-[12px] leading-tight">
          {label && <span className="text-fg-3">{label} </span>}
          <span className={cx('font-medium tabular-nums', value >= 100 ? 'text-danger' : value >= 80 ? 'text-warn' : 'text-fg')}>{pct === undefined ? '—' : fmtPct(pct)}</span>
        </span>
        {sub && <span className="block truncate text-[11px] leading-tight text-fg-3">{sub}</span>}
      </span>
    </Tooltip>
  )
}
