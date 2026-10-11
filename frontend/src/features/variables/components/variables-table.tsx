import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Check, Copy, Eye, EyeOff, Pencil, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { IconButton } from '@/components/ui/icon-button'
import type { Scope } from '@/lib/api/paths'
import { notifyError, notifySuccess } from '@/lib/toast'
import { revealVariable, updateVariables, type VariableValue } from '@/features/variables/api'
import { VariableDialog, type VariableInput } from './variable-dialog'

type TableProps = { scope: Scope; service: string; keys: string[] }

export function VariablesTable({ scope, service, keys }: TableProps) {
  return (
    <ul className="divide-y divide-line border border-line bg-surface">
      {keys.map((key) => (
        <li key={key}>
          <VariableRow scope={scope} service={service} name={key} />
        </li>
      ))}
    </ul>
  )
}

function VariableRow({ scope, service, name }: { scope: Scope; service: string; name: string }) {
  const queryClient = useQueryClient()
  const [revealed, setRevealed] = useState<VariableValue | null>(null)
  const [editing, setEditing] = useState(false)
  const [copied, setCopied] = useState(false)
  const reveal = useMutation({
    mutationFn: () => revealVariable(scope, service, name),
    onSuccess: setRevealed,
    onError: (error) => notifyError(`Couldn't show ${name}`, error),
  })

  async function ensureValue(): Promise<VariableValue> {
    return revealed ?? (await reveal.mutateAsync())
  }

  async function copy() {
    try {
      const { value } = await ensureValue()
      await navigator.clipboard.writeText(value)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch (error) {
      notifyError(`Couldn't copy ${name}`, error)
    }
  }

  async function save(input: VariableInput) {
    await updateVariables(scope, service, { set: { [name]: input.value }, deploy: input.apply })
    setRevealed(null)
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] })
    notifySuccess(`Saved ${name}`, input.apply ? `${service} was redeployed with it.` : 'It applies on the next deploy.')
  }

  async function remove() {
    await updateVariables(scope, service, { unset: [name], deploy: true })
    await queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service] })
    notifySuccess(`Removed ${name}`)
  }

  const stale = revealed !== null && revealed.deployed !== revealed.value
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-mono text-ui font-medium text-fg">{name}</span>
          {stale && (
            <Badge tone="warn">{revealed.deployed == null ? 'Not deployed yet' : 'Changed since deploy'}</Badge>
          )}
        </div>
        <p className="mt-0.5 truncate font-mono text-xs text-fg-muted">
          {revealed ? revealed.value || <span className="italic">empty</span> : '••••••••••••'}
        </p>
      </div>
      <div className="flex items-center gap-0.5">
        <IconButton
          label={revealed ? `Hide ${name}` : `Show ${name}`}
          disabled={reveal.isPending}
          onClick={() => (revealed ? setRevealed(null) : reveal.mutate())}
        >
          {revealed ? <EyeOff aria-hidden className="size-4" /> : <Eye aria-hidden className="size-4" />}
        </IconButton>
        <IconButton label={`Copy ${name}`} onClick={() => void copy()}>
          {copied ? <Check aria-hidden className="size-4 text-ok" /> : <Copy aria-hidden className="size-4" />}
        </IconButton>
        <IconButton label={`Edit ${name}`} onClick={() => void ensureValue().then(() => setEditing(true), () => undefined)}>
          <Pencil aria-hidden className="size-4" />
        </IconButton>
        <ConfirmDialog
          trigger={
            <IconButton label={`Remove ${name}`} tone="danger">
              <Trash2 aria-hidden className="size-4" />
            </IconButton>
          }
          title={`Remove ${name}?`}
          description={`${service} is redeployed without it. Services that reference it would break, so birdd refuses if any do.`}
          confirmLabel="Remove variable"
          tone="danger"
          onConfirm={remove}
        />
      </div>
      {revealed && (
        <VariableDialog
          open={editing}
          onOpenChange={setEditing}
          existing={{ key: name, value: revealed.value }}
          onSubmit={save}
        />
      )}
    </div>
  )
}
