import { Dialog } from '@base-ui/react/dialog'
import { X } from 'lucide-react'
import type { ReactNode } from 'react'
import { IconButton } from './icon-button'

// title row shared by every dialog: a hairline under it, close on the right
export function DialogHeader({ title, description, closable = true }: { title: ReactNode; description?: ReactNode; closable?: boolean }) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-line px-5 py-4">
      <div className="flex min-w-0 flex-col gap-1">
        <Dialog.Title className="text-title font-semibold tracking-tight text-fg">{title}</Dialog.Title>
        {description && <Dialog.Description className="text-sm text-fg-muted">{description}</Dialog.Description>}
      </div>
      {closable && (
        <Dialog.Close render={<IconButton label="Close" className="-mr-1.5" />}>
          <X aria-hidden className="size-4" />
        </Dialog.Close>
      )}
    </div>
  )
}
