import { FileDown, RotateCw, ScrollText, SquareTerminal, X } from 'lucide-react'
import { useState } from 'react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Alert } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { ChoiceCard, ChoiceGroup } from '@/components/ui/choice-card'
import { hintClass, labelClass } from '@/components/ui/control-styles'
import type { Scope } from '@/lib/api/paths'
import { command } from '@/lib/validation'
import type { CommandOptions, Target } from '@/features/terminal/connect'
import { OutputSession } from './output-session'
import { PipeSession } from './pipe-session'
import { type SessionEnd, TerminalSession } from './terminal-session'

type Mode = 'terminal' | 'output' | 'file'

const schema = z.object({
  target: z.enum(['exec', 'run']),
  mode: z.enum(['terminal', 'output', 'file']),
  machine: z.string(),
  skipEntrypoint: z.boolean(),
  command,
  download: z.string(),
})

type Session = {
  id: number
  mode: Mode
  target: Target
  command: string[]
  options: CommandOptions
  input: File | null
  downloadName: string
}

const targets: { value: Target; label: string; description: string }[] = [
  { value: 'exec', label: 'Running machine', description: 'Inspect what the app sees right now.' },
  { value: 'run', label: 'New container', description: 'A fresh one from the same image, removed when done.' },
]

const modes: { value: Mode; label: string; description: string; icon: typeof SquareTerminal }[] = [
  { value: 'terminal', label: 'Terminal', description: 'Interactive, like ssh', icon: SquareTerminal },
  { value: 'output', label: 'Show output', description: 'Run it and watch what it prints', icon: ScrollText },
  { value: 'file', label: 'File', description: 'Send a file as input, download the output', icon: FileDown },
]

function quote(args: string[]): string {
  return args.map((arg) => (/[\s'"\\]/.test(arg) || arg === '' ? JSON.stringify(arg) : arg)).join(' ')
}

type TerminalPanelProps = { scope: Scope; service: string; machines: string[] }

export function TerminalPanel({ scope, service, machines }: TerminalPanelProps) {
  const canExec = machines.length > 0
  const [input, setInput] = useState<File | null>(null)
  const [session, setSession] = useState<Session | null>(null)
  const [end, setEnd] = useState<SessionEnd | null>(null)
  const [count, setCount] = useState(0)

  const form = useAppForm({
    defaultValues: {
      target: (canExec ? 'exec' : 'run') as Target,
      mode: 'terminal' as Mode,
      machine: '',
      skipEntrypoint: false,
      command: 'sh',
      download: '',
    },
    validators: { onChange: schema },
    onSubmit: ({ value }) => {
      const values = schema.parse(value)
      setEnd(null)
      setCount(count + 1)
      setSession({
        id: count + 1,
        mode: values.mode,
        target: values.target,
        command: values.command,
        options: { machine: values.target === 'exec' ? values.machine : undefined, skipEntrypoint: values.target === 'run' && values.skipEntrypoint },
        input: values.mode === 'file' ? input : null,
        downloadName: values.download.trim(),
      })
    },
  })

  if (session) {
    const props = {
      scope,
      service,
      target: session.target,
      command: session.command,
      options: session.options,
      onEnd: setEnd,
    }
    return (
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <p className="min-w-0 text-sm text-fg-muted">
            <span className="font-mono text-fg">{quote(session.command)}</span> in{' '}
            {session.target === 'exec' ? (session.options.machine ? `machine ${session.options.machine.slice(0, 8)}` : 'a running machine') : 'a new container'}
          </p>
          <div className="flex gap-2">
            {end && (
              <Button variant="secondary" size="sm" onClick={() => setSession({ ...session, id: session.id + 1000 })}>
                <RotateCw aria-hidden className="size-4" />
                Run again
              </Button>
            )}
            <Button variant="ghost" size="sm" onClick={() => setSession(null)}>
              <X aria-hidden className="size-4" />
              {end ? 'Close' : 'Stop'}
            </Button>
          </div>
        </div>
        {end &&
          (end.kind === 'exit' ? (
            <p role="status" className="text-sm text-fg-muted">
              Exited with code <span className={`font-mono ${end.code === 0 ? 'text-ok' : 'text-bad'}`}>{end.code}</span>.
            </p>
          ) : (
            <Alert>{end.message}</Alert>
          ))}
        <div className="h-[min(32rem,60dvh)] overflow-hidden border border-line bg-sunken p-2">
          {session.mode === 'terminal' && <TerminalSession key={session.id} {...props} />}
          {session.mode === 'output' && <OutputSession key={session.id} {...props} />}
          {session.mode === 'file' && <PipeSession key={session.id} {...props} input={session.input} downloadName={session.downloadName} />}
        </div>
      </div>
    )
  }

  return (
    <form.AppForm>
      <form.FormElement className="flex max-w-2xl flex-col gap-6">
        <form.AppField name="target">
          {(targetField) => (
            <>
              <ChoiceGroup legend="Where">
                {targets.map((option) => {
                  const disabled = option.value === 'exec' && !canExec
                  return (
                    <ChoiceCard
                      key={option.value}
                      name="target"
                      label={option.label}
                      description={disabled ? 'No machine is running.' : option.description}
                      checked={targetField.state.value === option.value}
                      disabled={disabled}
                      onSelect={() => targetField.handleChange(option.value)}
                    />
                  )
                })}
              </ChoiceGroup>
              {targetField.state.value === 'exec' && machines.length > 1 && (
                <form.AppField name="machine">
                  {(field) => <field.SelectField label="Machine" options={[{ value: '', label: 'Any running machine' }, ...machines.map((id) => ({ value: id, label: id.slice(0, 13) }))]} />}
                </form.AppField>
              )}
              {targetField.state.value === 'run' && (
                <div className="-mt-2">
                  <form.AppField name="skipEntrypoint">
                    {(field) => (
                      <field.CheckboxField
                        label="Skip the image's entrypoint"
                        description="Run the program itself, for images whose entrypoint would treat the command as arguments."
                      />
                    )}
                  </form.AppField>
                </div>
              )}
            </>
          )}
        </form.AppField>
        <form.AppField name="mode">
          {(modeField) => (
            <>
              <ChoiceGroup legend="How">
                {modes.map((option) => (
                  <ChoiceCard
                    key={option.value}
                    name="mode"
                    label={option.label}
                    description={option.description}
                    icon={<option.icon aria-hidden className="size-4" />}
                    checked={modeField.state.value === option.value}
                    onSelect={() => modeField.handleChange(option.value)}
                  />
                ))}
              </ChoiceGroup>
              <form.AppField name="command">
                {(field) => (
                  <field.TextField
                    label="Command"
                    placeholder={modeField.state.value === 'file' ? 'pg_dump -Fc app' : 'bin/rails db:migrate'}
                    description={'Quote arguments with spaces, like psql -c "select 1". Nothing is expanded.'}
                    mono
                    autoComplete="off"
                    autoCapitalize="none"
                    spellCheck={false}
                  />
                )}
              </form.AppField>
              {modeField.state.value === 'file' && (
                <div className="grid gap-4 sm:grid-cols-2">
                  <label className="flex flex-col gap-1.5">
                    <span className={labelClass}>Input file (optional)</span>
                    <input
                      type="file"
                      onChange={(event) => setInput(event.target.files?.[0] ?? null)}
                      className="text-ui text-fg-muted file:mr-3 file:border file:border-line file:bg-surface file:px-2.5 file:py-1.5 file:font-mono file:text-2xs file:tracking-[0.08em] file:text-fg file:uppercase hover:file:bg-raised"
                    />
                    <span className={hintClass}>Sent as the command&apos;s stdin, like a dump to restore.</span>
                  </label>
                  <form.AppField name="download">
                    {(field) => <field.TextField label="Save output as" placeholder="app.dump" description="The download's file name." autoComplete="off" spellCheck={false} />}
                  </form.AppField>
                </div>
              )}
              <div>
                <form.SubmitButton>{modeField.state.value === 'terminal' ? 'Open terminal' : 'Run'}</form.SubmitButton>
              </div>
            </>
          )}
        </form.AppField>
      </form.FormElement>
    </form.AppForm>
  )
}
