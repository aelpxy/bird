import type { ComponentProps } from 'react'
import { Button } from '@/components/ui/button'
import { useFormContext } from './form-context'

// shows the wait while the form's submit handler runs
export function SubmitButton(props: Omit<ComponentProps<typeof Button>, 'type' | 'loading'>) {
  const form = useFormContext()
  return <form.Subscribe selector={(state) => state.isSubmitting}>{(submitting) => <Button {...props} type="submit" loading={submitting} />}</form.Subscribe>
}
