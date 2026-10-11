import { OTPField } from '@base-ui/react/otp-field'
import type { ReactNode } from 'react'
import { FieldShell } from './field-shell'
import { useFieldContext } from './form-context'

const CODE_LENGTH = 6

type CodeFieldProps = { label: string; description?: ReactNode; disabled?: boolean; autoFocus?: boolean }

// six digit boxes that send the form once the last one is filled, also when pasted
export function CodeField({ label, description, disabled, autoFocus = true }: CodeFieldProps) {
  const field = useFieldContext<string>()
  return (
    <FieldShell label={label} description={description}>
      {(control) => (
        <OTPField.Root
          id={control.id}
          aria-describedby={control['aria-describedby']}
          length={CODE_LENGTH}
          inputMode="numeric"
          value={field.state.value}
          onValueChange={(value) => field.handleChange(value)}
          onValueComplete={() => void field.form.handleSubmit()}
          disabled={disabled}
          className="flex gap-2"
        >
          {Array.from({ length: CODE_LENGTH }, (_, index) => (
            <OTPField.Input
              key={index}
              autoFocus={autoFocus && index === 0}
              onBlur={field.handleBlur}
              aria-invalid={control['aria-invalid']}
              aria-label={index === 0 ? undefined : `Digit ${index + 1} of ${CODE_LENGTH}`}
              className="h-12 w-full min-w-0 border border-line bg-sunken text-center font-mono text-lg font-medium text-fg transition-colors hover:border-line-strong focus:border-fg focus:outline-none aria-invalid:border-bad data-disabled:opacity-50"
            />
          ))}
        </OTPField.Root>
      )}
    </FieldShell>
  )
}
