import { ChevronDown } from 'lucide-react'
import type { ReactNode } from 'react'
import { inputClass } from '@/components/ui/control-styles'
import { FieldShell } from './field-shell'
import { useFieldContext } from './form-context'

type SelectFieldProps<T extends string | number> = {
  label: string
  options: { value: T; label: string }[]
  description?: ReactNode
  hideLabel?: boolean
  disabled?: boolean
}

// a native select: the platform's own picker is the most usable one on every device
export function SelectField<T extends string | number>({ label, options, description, hideLabel, disabled }: SelectFieldProps<T>) {
  const field = useFieldContext<T>()
  return (
    <FieldShell label={label} description={description} hideLabel={hideLabel}>
      {(control) => (
        <div className="relative">
          <select
            {...control}
            name={field.name}
            disabled={disabled}
            value={field.state.value}
            onChange={(event) => {
              const picked = options.find((option) => String(option.value) === event.target.value)
              if (picked) field.handleChange(picked.value)
            }}
            onBlur={field.handleBlur}
            className={`${inputClass} appearance-none pr-9`}
          >
            {options.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <ChevronDown aria-hidden className="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-fg-faint" />
        </div>
      )}
    </FieldShell>
  )
}
