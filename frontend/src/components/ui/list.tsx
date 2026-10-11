import type { ReactNode } from 'react'

// rows of things in one bordered box, like services, tokens or members
export function List({ children, label }: { children: ReactNode; label?: string }) {
  return (
    <ul aria-label={label} className="divide-y divide-line border border-line bg-surface">
      {children}
    </ul>
  )
}

export function ListItem({ children, className = '' }: { children: ReactNode; className?: string }) {
  return <li className={`flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 ${className}`}>{children}</li>
}

// the title and quieter detail line every row has
export function ListText({ title, detail }: { title: ReactNode; detail?: ReactNode }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="flex flex-wrap items-center gap-2 text-sm font-medium text-fg">{title}</div>
      {detail && <div className="mt-0.5 text-xs text-fg-faint">{detail}</div>}
    </div>
  )
}
