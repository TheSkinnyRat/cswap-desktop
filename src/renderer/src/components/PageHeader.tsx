import type { ReactNode } from 'react'

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: ReactNode; actions?: ReactNode }): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3 px-6 pb-3 pt-5">
      <div className="min-w-0">
        <h1 className="text-[16px] font-semibold tracking-tight text-fg">{title}</h1>
        {subtitle && <div className="mt-0.5 text-[12.5px] text-fg-2">{subtitle}</div>}
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>
    </div>
  )
}
