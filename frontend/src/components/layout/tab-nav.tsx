import type { ReactNode } from 'react'

// a row of links styled as tabs; each section keeps its own url
export function TabNav({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav aria-label={label} className="overflow-x-auto border-b border-line">
      <ul className="flex min-w-max">{children}</ul>
    </nav>
  )
}

export const tabLinkClass =
  'relative block px-3 py-2.5 text-ui text-fg-faint transition-colors hover:text-fg focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-2 focus-visible:outline-fg data-[status=active]:text-fg data-[status=active]:after:absolute data-[status=active]:after:inset-x-0 data-[status=active]:after:-bottom-px data-[status=active]:after:h-px data-[status=active]:after:bg-fg'
