import type { ComponentProps, ReactNode } from 'react'
import { inputClass } from '@/components/ui/control-styles'
import { FieldShell } from './field-shell'
import { useFieldContext } from './form-context'

type TextFieldProps = Omit<ComponentProps<'input'>, 'value' | 'onChange' | 'onBlur' | 'name' | 'className'> & {
  label: string
  description?: ReactNode
  hideLabel?: boolean
  // for values that are code, like variable names, images and commands
  mono?: boolean
  // a control inside the input's right edge, like a show-password toggle
  trailing?: ReactNode
}

export function TextField({ label, description, hideLabel, mono = false, trailing, ...input }: TextFieldProps) {
  const field = useFieldContext<string>()
  return (
    <FieldShell label={label} description={description} hideLabel={hideLabel}>
      {(control) => (
        <div className="relative">
          <input
            {...input}
            {...control}
            name={field.name}
            value={field.state.value}
            onChange={(event) => field.handleChange(event.target.value)}
            onBlur={field.handleBlur}
            className={`${inputClass} ${trailing ? 'pr-10' : ''} ${mono ? 'font-mono' : ''}`}
          />
          {trailing && <div className="absolute inset-y-0 right-0 flex items-center pr-1">{trailing}</div>}
        </div>
      )}
    </FieldShell>
  )
}
