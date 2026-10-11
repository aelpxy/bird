import { Checkbox } from '@base-ui/react/checkbox'
import { Check } from 'lucide-react'
import type { ReactNode } from 'react'
import { focusRing } from './control-styles'

type CheckboxFieldProps = {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
  label: ReactNode
  description?: ReactNode
  tone?: 'default' | 'danger'
}

export function CheckboxField({ checked, onCheckedChange, label, description, tone = 'default' }: CheckboxFieldProps) {
  const on = tone === 'danger' ? 'data-checked:border-bad data-checked:bg-bad' : 'data-checked:border-fg data-checked:bg-fg'
  return (
    <label className="flex cursor-pointer items-start gap-2.5 text-sm text-fg">
      <Checkbox.Root
        checked={checked}
        onCheckedChange={(next) => onCheckedChange(next)}
        className={`mt-0.5 grid size-4 shrink-0 place-items-center border border-line-strong bg-sunken text-canvas transition-colors ${on} ${focusRing}`}
      >
        <Checkbox.Indicator className="data-unchecked:hidden">
          <Check aria-hidden strokeWidth={3} className="size-3" />
        </Checkbox.Indicator>
      </Checkbox.Root>
      <span>
        {label}
        {description && <span className="mt-0.5 block text-xs text-fg-faint">{description}</span>}
      </span>
    </label>
  )
}
