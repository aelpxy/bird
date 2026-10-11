import type { ReactNode } from 'react'
import { eyebrowClass } from './control-styles'
import { tableRowClass } from './table-styles'

export type Column = { label: string; className?: string }

export function Table({ label, columns, layout, children }: { label: string; columns: Column[]; layout: string; children: ReactNode }) {
  return (
    <div role="table" aria-label={label} className="border border-line bg-surface">
      <div role="rowgroup" className="hidden border-b border-line sm:block">
        <div role="row" className={`${tableRowClass(layout)} py-2`}>
          {columns.map((column) => (
            <span key={column.label} role="columnheader" className={`${eyebrowClass} ${column.className ?? ''}`}>
              {column.label}
            </span>
          ))}
        </div>
      </div>
      <div role="rowgroup" className="divide-y divide-line">
        {children}
      </div>
    </div>
  )
}

export function TableRow({ layout, className = '', children }: { layout: string; className?: string; children: ReactNode }) {
  return (
    <div role="row" className={`${tableRowClass(layout)} py-3 ${className}`}>
      {children}
    </div>
  )
}

// cells after the first two take a full line on narrow screens, labelled since the header is hidden there
export function TableCell({ label, className = '', children }: { label?: string; className?: string; children: ReactNode }) {
  return (
    <div role="cell" className={`min-w-0 ${label ? 'max-sm:col-span-2 max-sm:flex max-sm:items-center max-sm:gap-3' : ''} ${className}`}>
      {label && <span className={`w-20 shrink-0 sm:hidden ${eyebrowClass}`}>{label}</span>}
      {children}
    </div>
  )
}
