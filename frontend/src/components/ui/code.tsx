import type { ReactNode } from 'react'

// inline code, commands and identifiers in running text
export function Code({ children }: { children: ReactNode }) {
  return <code className="border border-line bg-sunken px-1 py-px font-mono text-[0.92em] text-fg">{children}</code>
}
