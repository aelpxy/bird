import type { ReactNode } from 'react'
import { eyebrowClass } from './control-styles'

type PageHeaderProps = {
  // a small label above the title, like what kind of thing the page is about
  eyebrow?: ReactNode
  title: ReactNode
  description?: ReactNode
  actions?: ReactNode
}

export function PageHeader({ eyebrow, title, description, actions }: PageHeaderProps) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        {eyebrow && <p className={`${eyebrowClass} mb-2`}>{eyebrow}</p>}
        <h1 className="truncate text-2xl font-semibold tracking-tight text-fg">{title}</h1>
        {description && <div className="mt-1 text-sm text-fg-muted">{description}</div>}
      </div>
      {actions && <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>}
    </div>
  )
}
