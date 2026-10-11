import { Download } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { CheckboxField } from '@/components/ui/checkbox-field'
import { CopyButton } from '@/components/ui/copy-button'
import { Panel } from '@/components/ui/panel'

// shown once: numbered so a used one is easy to cross off, kept until the person says they saved them
export function RecoveryCodes({ user, codes, onDone }: { user: string; codes: string[]; onDone: () => void }) {
  const [saved, setSaved] = useState(false)
  const text = codes.join('\n')

  function download() {
    const body = `bird recovery codes for ${user}\nEach works once, in place of a code from your authenticator.\n\n${text}\n`
    const url = URL.createObjectURL(new Blob([body], { type: 'text/plain' }))
    Object.assign(document.createElement('a'), { href: url, download: `bird-recovery-codes-${user}.txt` }).click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="flex flex-col gap-5">
      <div>
        <h3 className="text-title font-semibold text-fg">Save your recovery codes</h3>
        <p className="mt-1 text-sm text-fg-muted">
          If you lose your phone, each of these signs you in once. This is the only time they are shown, so keep them
          somewhere other than this device, like a password manager.
        </p>
      </div>
      <Panel tone="notice" padded={false}>
        <ol className="grid grid-cols-1 gap-px bg-line sm:grid-cols-2">
          {codes.map((code, index) => (
            <li key={code} className="flex items-center gap-3 bg-sunken px-4 py-2.5 font-mono text-sm">
              <span className="tabular w-5 text-right text-2xs text-fg-faint">{index + 1}</span>
              <span className="tracking-[0.06em] text-fg select-all">{code}</span>
            </li>
          ))}
        </ol>
      </Panel>
      <div className="flex flex-wrap gap-2">
        <CopyButton value={text} label="Copy all" />
        <Button variant="secondary" size="sm" onClick={download}>
          <Download aria-hidden className="size-3.5" />
          Download .txt
        </Button>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-4 border-t border-line pt-5">
        <CheckboxField checked={saved} onCheckedChange={setSaved} label="I saved these codes somewhere safe" />
        <Button disabled={!saved} onClick={onDone}>
          Done
        </Button>
      </div>
    </div>
  )
}
