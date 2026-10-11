import { createFormHook } from '@tanstack/react-form'
import { CheckboxField } from './checkbox-field'
import { CodeField } from './code-field'
import { FormElement } from './form-element'
import { fieldContext, formContext } from './form-context'
import { NumberField } from './number-field'
import { PasswordField } from './password-field'
import { SelectField } from './select-field'
import { SubmitButton } from './submit-button'
import { TextareaField } from './textarea-field'
import { TextField } from './text-field'

// every form in the app: fields come pre-bound, so a form only says which field shows which value
export const { useAppForm, withForm } = createFormHook({
  fieldContext,
  formContext,
  fieldComponents: { TextField, PasswordField, TextareaField, NumberField, SelectField, CheckboxField, CodeField },
  formComponents: { SubmitButton, FormElement },
})
