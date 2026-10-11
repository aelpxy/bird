import { useId, type ReactNode } from 'react'
import { eyebrowClass } from './control-styles'

type SectionProps = { title: string; description?: ReactNode; actions?: ReactNode; children: ReactNode }

// a titled part of a page: a mono label over a hairline, then the content
export function Section({ title, description, actions, children }: SectionProps) {
  const id = useId()
  return (
    <section aria-labelledby={id} className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3 border-b border-line pb-2.5">
        <div>
          <h2 id={id} className={eyebrowClass}>
            {title}
          </h2>
          {description && <p className="mt-1.5 text-sm text-fg-muted">{description}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
      {children}
    </section>
  )
}
