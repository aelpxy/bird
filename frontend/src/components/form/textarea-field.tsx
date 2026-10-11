import type { ComponentProps, ReactNode } from 'react'
import { textareaClass } from '@/components/ui/control-styles'
import { FieldShell } from './field-shell'
import { useFieldContext } from './form-context'

type TextareaFieldProps = Omit<ComponentProps<'textarea'>, 'value' | 'onChange' | 'onBlur' | 'name' | 'className'> & {
  label: string
  description?: ReactNode
}

// multi-line values, always in mono since they are usually code or config
export function TextareaField({ label, description, rows = 3, ...textarea }: TextareaFieldProps) {
  const field = useFieldContext<string>()
  return (
    <FieldShell label={label} description={description}>
      {(control) => (
        <textarea
          {...textarea}
          {...control}
          rows={rows}
          spellCheck={false}
          name={field.name}
          value={field.state.value}
          onChange={(event) => field.handleChange(event.target.value)}
          onBlur={field.handleBlur}
          className={textareaClass}
        />
      )}
    </FieldShell>
  )
}
