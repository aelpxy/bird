import { Toast } from '@base-ui/react/toast'
import { CircleAlert, CircleCheck, X } from 'lucide-react'
import type { ReactNode } from 'react'
import { toastManager } from '@/lib/toast'
import { IconButton } from './icon-button'

export function Toaster({ children }: { children: ReactNode }) {
  return (
    <Toast.Provider toastManager={toastManager}>
      {children}
      <Toast.Portal>
        <Toast.Viewport className="fixed right-4 bottom-4 z-50 flex w-[calc(100vw-2rem)] flex-col-reverse gap-2 sm:w-96">
          <Toasts />
        </Toast.Viewport>
      </Toast.Portal>
    </Toast.Provider>
  )
}

function Toasts() {
  const { toasts } = Toast.useToastManager()
  return toasts.map((toast) => {
    const failed = toast.type === 'error'
    const Icon = failed ? CircleAlert : CircleCheck
    return (
      <Toast.Root
        key={toast.id}
        toast={toast}
        className={`flex items-start gap-3 border border-line-strong border-l-2 bg-surface p-3 transition-[opacity,translate] duration-150 ease-out data-ending-style:translate-x-4 data-ending-style:opacity-0 data-starting-style:translate-x-4 data-starting-style:opacity-0 ${failed ? 'border-l-bad' : 'border-l-ok'}`}
      >
        <Icon aria-hidden className={`mt-0.5 size-4 shrink-0 ${failed ? 'text-bad' : 'text-ok'}`} />
        <Toast.Content className="flex min-w-0 flex-1 flex-col gap-0.5">
          <Toast.Title className="text-sm font-medium text-fg" />
          <Toast.Description className="text-ui text-fg-muted" />
        </Toast.Content>
        <Toast.Close render={<IconButton label="Dismiss" className="-m-1" />}>
          <X aria-hidden className="size-3.5" />
        </Toast.Close>
      </Toast.Root>
    )
  })
}
