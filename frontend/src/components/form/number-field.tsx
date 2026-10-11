import type { ComponentProps, ReactNode } from 'react'
import { inputClass } from '@/components/ui/control-styles'
import { FieldShell } from './field-shell'
import { useFieldContext } from './form-context'

type NumberFieldProps = Omit<ComponentProps<'input'>, 'value' | 'onChange' | 'onBlur' | 'name' | 'className' | 'type'> & {
  label: string
  description?: ReactNode
}

// an empty box is NaN, which the schema reports as missing
export function NumberField({ label, description, inputMode = 'numeric', ...input }: NumberFieldProps) {
  const field = useFieldContext<number>()
  return (
    <FieldShell label={label} description={description}>
      {(control) => (
        <input
          {...input}
          {...control}
          type="number"
          inputMode={inputMode}
          name={field.name}
          value={Number.isNaN(field.state.value) ? '' : field.state.value}
          onChange={(event) => field.handleChange(event.target.valueAsNumber)}
          onBlur={field.handleBlur}
          className={`${inputClass} tabular font-mono`}
        />
      )}
    </FieldShell>
  )
}
