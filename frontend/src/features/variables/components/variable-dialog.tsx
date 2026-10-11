import type { ReactElement } from 'react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Code } from '@/components/ui/code'
import { FormDialog } from '@/components/ui/form-dialog'
import { envKey } from '@/lib/validation'

export type VariableInput = { key: string; value: string; apply: boolean }

type VariableDialogProps = {
  trigger?: ReactElement
  open?: boolean
  onOpenChange?: (open: boolean) => void
  // editing keeps the name; adding asks for one
  existing?: { key: string; value: string }
  onSubmit: (input: VariableInput) => Promise<unknown>
}

const schema = z.object({ key: envKey, value: z.string(), apply: z.boolean() })

export function VariableDialog({ trigger, open, onOpenChange, existing, onSubmit }: VariableDialogProps) {
  const dialog = useDialogForm({
    schema,
    defaultValues: { key: existing?.key ?? '', value: existing?.value ?? '', apply: true },
    open,
    onOpenChange,
    onSubmit,
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={trigger}
      title={existing ? `Edit ${existing.key}` : 'New variable'}
      description={
        <>
          Reference other variables with <Code>{'${{KEY}}'}</Code> or another service&apos;s with{' '}
          <Code>{'${{postgres.DATABASE_URL}}'}</Code>.
        </>
      }
      submitLabel={existing ? 'Save' : 'Add variable'}
    >
      {!existing && (
        <dialog.form.AppField name="key">
          {(field) => <field.TextField label="Name" placeholder="DATABASE_URL" mono autoFocus autoComplete="off" autoCapitalize="characters" spellCheck={false} />}
        </dialog.form.AppField>
      )}
      <dialog.form.AppField name="value">
        {(field) => <field.TextareaField label="Value" autoFocus={Boolean(existing)} autoComplete="off" />}
      </dialog.form.AppField>
      <dialog.form.AppField name="apply">
        {(field) => <field.CheckboxField label="Redeploy now" description="Otherwise the change waits for the next deploy." />}
      </dialog.form.AppField>
    </FormDialog>
  )
}
