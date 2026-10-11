import type { ComponentProps } from 'react'
import { useFormContext } from './form-context'

// the native form element, sending through TanStack Form so the schema checks every field first
export function FormElement({ children, ...props }: Omit<ComponentProps<'form'>, 'onSubmit' | 'noValidate'>) {
  const form = useFormContext()
  return (
    <form
      {...props}
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        event.stopPropagation()
        void form.handleSubmit()
      }}
    >
      {children}
    </form>
  )
}
