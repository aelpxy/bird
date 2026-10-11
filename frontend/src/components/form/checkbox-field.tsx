import type { ReactNode } from 'react'
import { CheckboxField as Checkbox } from '@/components/ui/checkbox-field'
import { useFieldContext } from './form-context'

type CheckboxFieldProps = { label: ReactNode; description?: ReactNode; tone?: 'default' | 'danger' }

export function CheckboxField(props: CheckboxFieldProps) {
  const field = useFieldContext<boolean>()
  return <Checkbox {...props} checked={field.state.value} onCheckedChange={field.handleChange} />
}
