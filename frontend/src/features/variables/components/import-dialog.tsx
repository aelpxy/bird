import { FileUp } from 'lucide-react'
import { z } from 'zod'
import { useDialogForm } from '@/components/form/use-dialog-form'
import { Button } from '@/components/ui/button'
import { FormDialog } from '@/components/ui/form-dialog'
import { plural } from '@/lib/format'
import { parseDotenv } from '../dotenv'

// the pasted text becomes the variables it holds, or the line that is wrong
const dotenv = z.string().transform((text, context) => {
  const parsed = parseDotenv(text)
  if (!parsed.ok) {
    context.addIssue({ code: 'custom', message: parsed.error })
    return z.NEVER
  }
  if (Object.keys(parsed.values).length === 0) {
    context.addIssue({ code: 'custom', message: 'Paste at least one KEY=value line.' })
    return z.NEVER
  }
  return parsed.values
})

const schema = z.object({ dotenv, apply: z.boolean() })

type ImportDialogProps = { onImport: (values: Record<string, string>, apply: boolean) => Promise<unknown> }

// many variables at once, pasted from a .env file
export function ImportDialog({ onImport }: ImportDialogProps) {
  const dialog = useDialogForm({
    schema,
    defaultValues: { dotenv: '', apply: true },
    onSubmit: (values) => onImport(values.dotenv, values.apply),
  })
  return (
    <FormDialog
      dialog={dialog}
      trigger={
        <Button variant="secondary" size="sm">
          <FileUp aria-hidden className="size-3.5" />
          Import .env
        </Button>
      }
      size="lg"
      title="Import variables"
      description="Paste KEY=value lines. Names that exist already get the new value; others stay as they are."
      submitLabel="Save variables"
    >
      <dialog.form.AppField name="dotenv">
        {(field) => {
          const parsed = parseDotenv(field.state.value)
          const count = parsed.ok ? Object.keys(parsed.values).length : 0
          return (
            <field.TextareaField
              label=".env contents"
              rows={10}
              autoFocus
              placeholder={'DATABASE_URL=${{postgres.DATABASE_URL}}\nLOG_LEVEL=info'}
              description={count > 0 ? `${plural(count, 'variable')} found.` : 'Comments and blank lines are skipped.'}
            />
          )
        }}
      </dialog.form.AppField>
      <dialog.form.AppField name="apply">
        {(field) => <field.CheckboxField label="Redeploy now" description="Otherwise the changes wait for the next deploy." />}
      </dialog.form.AppField>
    </FormDialog>
  )
}
