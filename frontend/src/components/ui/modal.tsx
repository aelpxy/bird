import { Dialog } from '@base-ui/react/dialog'
import type { ReactNode } from 'react'
import { DialogHeader } from './dialog-header'
import { backdropClass, popupClass } from './overlay-styles'

type ModalProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: ReactNode
  description?: ReactNode
  children: ReactNode
}

// a dialog that shows something, like a run's output, rather than asking for input
export function Modal({ open, onOpenChange, title, description, children }: ModalProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Backdrop className={backdropClass} />
        <Dialog.Popup className={popupClass('xl')}>
          <DialogHeader title={title} description={description} />
          <div className="flex flex-col gap-4 px-5 py-5">{children}</div>
        </Dialog.Popup>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
