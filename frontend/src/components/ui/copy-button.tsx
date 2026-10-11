import { Check, Copy } from 'lucide-react'
import { useState } from 'react'
import { notifyError } from '@/lib/toast'
import { Button } from './button'

export function CopyButton({ value, label = 'Copy' }: { value: string; label?: string }) {
  const [copied, setCopied] = useState(false)
  async function copy() {
    try {
      await navigator.clipboard.writeText(value)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch (error) {
      notifyError("Couldn't copy", error)
    }
  }
  return (
    <Button type="button" variant="secondary" size="sm" onClick={() => void copy()}>
      {copied ? <Check aria-hidden className="size-3.5 text-ok" /> : <Copy aria-hidden className="size-3.5" />}
      <span aria-live="polite">{copied ? 'Copied' : label}</span>
    </Button>
  )
}
