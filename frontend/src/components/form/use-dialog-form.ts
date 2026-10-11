import type { StandardSchemaV1 } from '@tanstack/react-form'
import { useState } from 'react'
import type { z } from 'zod'
import { useAppForm } from './app-form'

type DialogFormOptions<Schema extends z.ZodType> = {
  schema: Schema
  defaultValues: z.input<Schema>
  // the dialog closes once this resolves and stays open with the error when it rejects
  onSubmit: (values: z.output<Schema>) => Promise<unknown>
  // for dialogs something else opens, like a menu item
  open?: boolean
  onOpenChange?: (open: boolean) => void
}

// a form inside a dialog: validated by its schema, reset whenever the dialog closes
export function useDialogForm<Schema extends z.ZodType>({ schema, defaultValues, onSubmit, open: controlledOpen, onOpenChange }: DialogFormOptions<Schema>) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(false)
  const open = controlledOpen ?? uncontrolledOpen
  function setOpenState(next: boolean) {
    setUncontrolledOpen(next)
    onOpenChange?.(next)
  }
  const [error, setError] = useState<unknown>(null)
  const [pending, setPending] = useState(false)
  // zod schemas are standard schemas; naming the type lets TanStack Form see this one is not async
  const validator: StandardSchemaV1<z.input<Schema>, unknown> = schema
  const form = useAppForm({
    defaultValues,
    validators: { onChange: validator },
    onSubmit: async ({ value }) => {
      setError(null)
      setPending(true)
      try {
        await onSubmit(schema.parse(value))
        setOpenState(false)
        form.reset()
      } catch (cause) {
        setError(cause)
      } finally {
        setPending(false)
      }
    },
  })

  function setOpen(next: boolean) {
    if (pending) return
    setOpenState(next)
    if (!next) {
      setError(null)
      form.reset()
    }
  }

  return { form, open, setOpen, error, pending }
}
