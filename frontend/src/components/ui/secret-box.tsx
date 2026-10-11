import { X } from 'lucide-react'
import type { ReactNode } from 'react'
import { CopyButton } from './copy-button'
import { IconButton } from './icon-button'
import { Panel } from './panel'

type SecretBoxProps = { value: string; title: string; children?: ReactNode; onDismiss?: () => void }

// something shown once, like a new token or recovery codes, with a way to keep it
export function SecretBox({ value, title, children, onDismiss }: SecretBoxProps) {
  return (
    <Panel tone="notice" className="flex flex-col gap-3">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-sm font-medium text-fg">{title}</p>
          {children && <div className="mt-1 text-sm text-fg-muted">{children}</div>}
        </div>
        {onDismiss && (
          <IconButton label="Dismiss" onClick={onDismiss}>
            <X aria-hidden className="size-4" />
          </IconButton>
        )}
      </div>
      <pre className="overflow-x-auto border border-line bg-sunken px-3 py-2.5 font-mono text-sm break-all whitespace-pre-wrap text-fg">{value}</pre>
      <div>
        <CopyButton value={value} />
      </div>
    </Panel>
  )
}
