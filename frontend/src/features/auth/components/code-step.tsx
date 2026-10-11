import { ArrowLeft } from 'lucide-react'
import { useState } from 'react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Button } from '@/components/ui/button'
import { focusRing } from '@/components/ui/control-styles'

type Mode = 'app' | 'recovery'

const schemas: Record<Mode, z.ZodType<{ code: string }, { code: string }>> = {
  app: z.object({ code: z.string().regex(/^\d{6}$/, 'Enter the 6 digits.') }),
  recovery: z.object({ code: z.string().trim().min(1, 'Enter one of your recovery codes.') }),
}

type CodeStepProps = {
  // changes after a wrong code, so the form starts empty again
  attempt: number
  onSubmit: (code: string) => Promise<void>
  onBack: () => void
}

// the authenticator's code by default; recovery codes are for a lost phone
export function CodeStep({ attempt, onSubmit, onBack }: CodeStepProps) {
  const [mode, setMode] = useState<Mode>('app')
  return (
    <div className="flex flex-col gap-4">
      <CodeForm key={`${mode}-${attempt}`} mode={mode} onSubmit={onSubmit} />
      <div className="flex items-center justify-between">
        <Button variant="ghost" size="sm" onClick={onBack} className="-ml-2.5">
          <ArrowLeft aria-hidden className="size-3.5" />
          Back
        </Button>
        <button
          type="button"
          onClick={() => setMode((current) => (current === 'app' ? 'recovery' : 'app'))}
          className={`text-ui text-fg-muted underline decoration-line-strong underline-offset-4 transition-colors hover:text-fg hover:decoration-fg ${focusRing}`}
        >
          {mode === 'app' ? 'Use a recovery code' : 'Use your authenticator app'}
        </button>
      </div>
    </div>
  )
}

function CodeForm({ mode, onSubmit }: { mode: Mode; onSubmit: (code: string) => Promise<void> }) {
  const schema = schemas[mode]
  const form = useAppForm({
    defaultValues: { code: '' },
    validators: { onSubmit: schema },
    onSubmit: ({ value }) => onSubmit(schema.parse(value).code),
  })
  return (
    <form.AppForm>
      <form.FormElement className="flex flex-col gap-4">
        <form.AppField name="code">
          {(field) =>
            mode === 'app' ? (
              <field.CodeField label="Authentication code" description="Enter the 6-digit code from your authenticator app." />
            ) : (
              <field.TextField label="Recovery code" placeholder="xxxx-xxxx" description="Each recovery code works once." mono autoFocus autoComplete="off" autoCapitalize="none" spellCheck={false} />
            )
          }
        </form.AppField>
        <form.SubmitButton className="w-full">Verify</form.SubmitButton>
      </form.FormElement>
    </form.AppForm>
  )
}
