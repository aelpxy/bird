import { Dialog } from '@base-ui/react/dialog'
import type { ComponentType, ReactElement, ReactNode } from 'react'
import { Button } from './button'
import { DialogHeader } from './dialog-header'
import { ErrorAlert } from './error-alert'
import { backdropClass, popupClass } from './overlay-styles'

// what a dialog needs from useDialogForm; the form's own fields stay typed where they are written
type DialogController = {
  form: {
    AppForm: ComponentType<{ children?: ReactNode }>
    FormElement: ComponentType<{ className?: string; children?: ReactNode }>
    SubmitButton: ComponentType<{ children?: ReactNode }>
  }
  open: boolean
  setOpen: (open: boolean) => void
  error: unknown
  pending: boolean
}

type FormDialogProps = {
  dialog: DialogController
  // left out when something else opens the dialog, like a menu item that closes its menu
  trigger?: ReactElement
  title: string
  description?: ReactNode
  submitLabel: string
  // shown while submitting, for work that takes a while, like a deploy
  pendingNote?: ReactNode
  size?: 'md' | 'lg'
  // the footer's left side, like a back button in a dialog with steps
  footerStart?: ReactNode
  // for a step that has nothing to submit yet
  hideSubmit?: boolean
  children: ReactNode
}

export function FormDialog({ dialog, trigger, title, description, submitLabel, pendingNote, size = 'md', footerStart, hideSubmit = false, children }: FormDialogProps) {
  const { form } = dialog
  return (
    <Dialog.Root open={dialog.open} onOpenChange={dialog.setOpen}>
      {trigger && <Dialog.Trigger render={trigger} />}
      <Dialog.Portal>
        <Dialog.Backdrop className={backdropClass} />
        <Dialog.Popup className={popupClass(size)}>
          <DialogHeader title={title} description={description} closable={!dialog.pending} />
          <form.AppForm>
            <form.FormElement className="flex flex-col">
              <div className="flex flex-col gap-4 px-5 py-5">
                {dialog.error !== null && <ErrorAlert error={dialog.error} />}
                {children}
                {dialog.pending && pendingNote && (
                  <p role="status" className="text-sm text-fg-muted">
                    {pendingNote}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-2 border-t border-line bg-canvas/40 px-5 py-3">
                <div className="flex-1">{footerStart}</div>
                <Dialog.Close render={<Button variant="ghost" disabled={dialog.pending} />}>Cancel</Dialog.Close>
                {!hideSubmit && <form.SubmitButton>{submitLabel}</form.SubmitButton>}
              </div>
            </form.FormElement>
          </form.AppForm>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
