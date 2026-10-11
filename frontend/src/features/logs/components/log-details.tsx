import { Check, Copy, Minus, Plus } from 'lucide-react'
import { useState } from 'react'
import { IconButton } from '@/components/ui/icon-button'
import type { ShownLine } from '../filters'
import { fieldTerm } from '../query'

type Props = { line: ShownLine; onAddTerm: (term: string) => void }

// everything known about one line, each field one click from the query
export function LogDetails({ line, onAddTerm }: Props) {
  const rows: [string, string][] = [
    ['time', line.at ? new Date(line.at).toISOString() : 'not recorded'],
    ['machine', line.machine],
    ['stream', line.stream],
    ...(line.level ? [['level', line.level] as [string, string]] : []),
    ...Object.entries(line.fields ?? {}).filter(([key]) => key !== 'level'),
  ]
  return (
    <div className="mx-3 my-1.5 border border-line bg-surface font-mono text-xs">
      <div className="flex items-center justify-end gap-0.5 border-b border-line px-2 py-1">
        <CopyButton label="Copy the line" value={line.plain} />
        {line.fields && <CopyButton label="Copy the fields as JSON" value={JSON.stringify(line.fields, null, 2)} />}
      </div>
      <dl className="divide-y divide-line">
        {rows.map(([key, value]) => (
          <div key={key} className="group/field flex items-start gap-3 px-3 py-1">
            <dt className="w-32 shrink-0 truncate text-fg-faint" title={key}>
              {key}
            </dt>
            <dd className="min-w-0 flex-1 break-all whitespace-pre-wrap text-fg">{value}</dd>
            {key !== 'time' && (
              <span className="flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover/field:opacity-100 focus-within:opacity-100">
                <IconButton label={`Only lines where ${key} is ${value}`} className="size-5" onClick={() => onAddTerm(fieldTerm(key, value))}>
                  <Plus aria-hidden className="size-3" />
                </IconButton>
                <IconButton label={`Hide lines where ${key} is ${value}`} className="size-5" onClick={() => onAddTerm(fieldTerm(key, value, true))}>
                  <Minus aria-hidden className="size-3" />
                </IconButton>
              </span>
            )}
          </div>
        ))}
      </dl>
    </div>
  )
}

function CopyButton({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false)
  return (
    <IconButton
      label={label}
      className="size-6"
      onClick={() => {
        void navigator.clipboard.writeText(value).then(() => {
          setCopied(true)
          window.setTimeout(() => setCopied(false), 1500)
        })
      }}
    >
      {copied ? <Check aria-hidden className="size-3.5 text-ok" /> : <Copy aria-hidden className="size-3.5" />}
    </IconButton>
  )
}
