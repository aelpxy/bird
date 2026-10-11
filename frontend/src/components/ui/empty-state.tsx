import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

type EmptyStateProps = {
  icon: LucideIcon
  title: string
  description?: ReactNode
  action?: ReactNode
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
  return (
    <div className="bg-grid flex flex-col items-center border border-dashed border-line-strong px-6 py-14 text-center">
      <div className="grid size-10 place-items-center border border-line-strong bg-surface">
        <Icon aria-hidden className="size-4 text-fg-muted" />
      </div>
      <p className="mt-4 text-sm font-medium text-fg">{title}</p>
      {description && <div className="mt-1 max-w-md text-sm text-fg-muted">{description}</div>}
      {action && <div className="mt-5">{action}</div>}
    </div>
  )
}
