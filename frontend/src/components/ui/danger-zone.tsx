import type { ReactNode } from 'react'
import { Panel } from './panel'

export function DangerZone({ title, description, action }: { title: string; description: ReactNode; action: ReactNode }) {
  return (
    <Panel tone="danger" className="flex flex-wrap items-center justify-between gap-4">
      <div>
        <p className="text-sm font-medium text-fg">{title}</p>
        <p className="mt-0.5 text-sm text-fg-muted">{description}</p>
      </div>
      {action}
    </Panel>
  )
}
