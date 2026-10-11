import { useId, type ReactNode } from 'react'
import { errorTextClass, hintClass, labelClass } from '@/components/ui/control-styles'
import { useFieldContext } from './form-context'

export type ControlProps = {
  id: string
  'aria-invalid': true | undefined
  'aria-describedby': string | undefined
}

type FieldShellProps = {
  label: string
  description?: ReactNode
  hideLabel?: boolean
  children: (control: ControlProps) => ReactNode
}

// label, hint and errors around any control; errors wait until the field was left or the form sent
export function FieldShell({ label, description, hideLabel = false, children }: FieldShellProps) {
  const field = useFieldContext<unknown>()
  const id = useId()
  const errors = visibleErrors(field)
  const descriptionId = description ? `${id}-description` : undefined
  const errorId = errors.length > 0 ? `${id}-error` : undefined
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className={hideLabel ? 'sr-only' : labelClass}>
        {label}
      </label>
      {children({
        id,
        'aria-invalid': errorId ? true : undefined,
        'aria-describedby': [errorId, descriptionId].filter(Boolean).join(' ') || undefined,
      })}
      {description && (
        <p id={descriptionId} className={hintClass}>
          {description}
        </p>
      )}
      {errorId && (
        <p id={errorId} className={errorTextClass}>
          {errors.join(' ')}
        </p>
      )}
    </div>
  )
}

type FieldLike = ReturnType<typeof useFieldContext<unknown>>

function visibleErrors(field: FieldLike): string[] {
  const shown = field.state.meta.isBlurred || field.form.state.submissionAttempts > 0
  if (!shown) return []
  const messages = field.state.meta.errors.map((error: unknown) =>
    typeof error === 'string' ? error : (error as { message?: string } | undefined)?.message ?? '',
  )
  return [...new Set(messages.filter(Boolean))]
}
