import { ChevronDown } from 'lucide-react'
import { useId } from 'react'
import { hintClass, inputClass, labelClass } from './control-styles'

type Option<T extends string | number> = { value: T; label: string }

type SelectFieldProps<T extends string | number> = {
  label: string
  value: T
  options: Option<T>[]
  onChange: (value: T) => void
  description?: string
  disabled?: boolean
  // for selects in a row of a table, where a visible label would repeat on every row
  hideLabel?: boolean
}

// a native select: the platform's own picker is the most usable one on every device
export function SelectField<T extends string | number>({ label, value, options, onChange, description, disabled, hideLabel = false }: SelectFieldProps<T>) {
  const id = useId()
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className={hideLabel ? 'sr-only' : labelClass}>
        {label}
      </label>
      <div className="relative">
        <select
          id={id}
          value={value}
          disabled={disabled}
          onChange={(event) => {
            const picked = options.find((option) => String(option.value) === event.target.value)
            if (picked) onChange(picked.value)
          }}
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
      {description && <p className={hintClass}>{description}</p>}
    </div>
  )
}
