import type { ReactNode } from 'react'
import { eyebrowClass } from '@/components/ui/control-styles'

// settings split into pages: a side list on wide screens, a row of tabs on narrow ones
export function SettingsLayout({ label, nav, children }: { label: string; nav: ReactNode; children: ReactNode }) {
  return (
    <div className="grid gap-8 md:grid-cols-[11rem_minmax(0,1fr)]">
      <nav aria-label={label} className="md:sticky md:top-20 md:self-start">
        <p className={`${eyebrowClass} mb-2 hidden md:block`}>{label}</p>
        <ul className="-mx-1 flex gap-1 overflow-x-auto border-b border-line pb-px md:mx-0 md:flex-col md:gap-0 md:overflow-visible md:border-b-0 md:border-l md:pb-0">
          {nav}
        </ul>
      </nav>
      <div className="min-w-0 max-w-3xl">{children}</div>
    </div>
  )
}

export const settingsLinkClass =
  'pressable block whitespace-nowrap px-3 py-1.5 text-ui text-fg-faint hover:text-fg focus-visible:outline focus-visible:outline-1 focus-visible:-outline-offset-1 focus-visible:outline-fg data-[status=active]:text-fg md:-ml-px md:border-l md:border-transparent md:data-[status=active]:border-fg md:data-[status=active]:bg-surface'
