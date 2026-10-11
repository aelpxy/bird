import { AlertDialog } from '@base-ui/react/alert-dialog'
import { useState, type ReactElement, type ReactNode } from 'react'
import { Button } from './button'
import { inputClass, labelClass } from './control-styles'
import { ErrorAlert } from './error-alert'
import { backdropClass, popupClass } from './overlay-styles'

type ConfirmDialogProps = {
  trigger: ReactElement
  title: string
  description: ReactNode
  confirmLabel: string
  tone?: 'danger' | 'default'
  // for actions that cannot be undone: confirming needs this typed exactly
  confirmText?: string
  children?: ReactNode
  // the dialog stays open with the error shown when this rejects
  onConfirm: () => Promise<unknown>
}

export function ConfirmDialog({ trigger, title, description, confirmLabel, tone = 'default', confirmText, children, onConfirm }: ConfirmDialogProps) {
  const [open, setOpen] = useState(false)
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const [typed, setTyped] = useState('')

  async function confirm() {
    setPending(true)
    setError(null)
    try {
      await onConfirm()
      setOpen(false)
    } catch (cause) {
      setError(cause)
    } finally {
      setPending(false)
    }
  }

  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(next) => {
        if (pending) return
        setOpen(next)
        if (!next) {
          setError(null)
          setTyped('')
        }
      }}
    >
      <AlertDialog.Trigger render={trigger} />
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className={backdropClass} />
        <AlertDialog.Popup className={`${popupClass('md')} ${tone === 'danger' ? 'border-t-2 border-t-bad' : ''}`}>
          <div className="flex flex-col gap-4 px-5 py-5">
            <div className="flex flex-col gap-1.5">
              <AlertDialog.Title className="text-title font-semibold tracking-tight text-fg">{title}</AlertDialog.Title>
              <AlertDialog.Description className="text-sm text-fg-muted">{description}</AlertDialog.Description>
            </div>
            {children}
            {confirmText && (
              <label className="flex flex-col gap-1.5">
                <span className={labelClass}>
                  Type <span className="font-mono text-fg">{confirmText}</span> to confirm
                </span>
                <input
                  value={typed}
                  onChange={(event) => setTyped(event.target.value)}
                  autoComplete="off"
                  autoCapitalize="none"
                  spellCheck={false}
                  className={`${inputClass} font-mono`}
                />
              </label>
            )}
            {error !== null && <ErrorAlert error={error} />}
          </div>
          <div className="flex justify-end gap-2 border-t border-line bg-canvas/40 px-5 py-3">
            <AlertDialog.Close render={<Button variant="ghost" disabled={pending} />}>Cancel</AlertDialog.Close>
            <Button
              variant={tone === 'danger' ? 'danger' : 'primary'}
              loading={pending}
              disabled={confirmText !== undefined && typed !== confirmText}
              onClick={() => void confirm()}
            >
              {confirmLabel}
            </Button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
