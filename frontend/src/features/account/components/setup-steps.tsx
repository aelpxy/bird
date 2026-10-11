import { Check } from 'lucide-react'

const STEPS = ['Scan', 'Verify', 'Save codes'] as const

// where the setup is: done steps get a check, the current one is solid white
export function SetupSteps({ current }: { current: 0 | 1 | 2 }) {
  return (
    <ol className="grid grid-cols-3 border border-line bg-surface" aria-label="Setup progress">
      {STEPS.map((label, index) => {
        const done = index < current
        const active = index === current
        return (
          <li
            key={label}
            aria-current={active ? 'step' : undefined}
            className={`flex items-center gap-2.5 px-3 py-2.5 ${index > 0 ? 'border-l border-line' : ''} ${active ? 'bg-raised' : ''}`}
          >
            <span
              className={`grid size-5 shrink-0 place-items-center border font-mono text-2xs ${active ? 'border-fg bg-fg text-canvas' : done ? 'border-fg text-fg' : 'border-line-strong text-fg-faint'}`}
            >
              {done ? <Check aria-hidden strokeWidth={3} className="size-3" /> : `0${index + 1}`}
            </span>
            <span className={`truncate text-ui ${active || done ? 'text-fg' : 'text-fg-faint'}`}>{label}</span>
          </li>
        )
      })}
    </ol>
  )
}
