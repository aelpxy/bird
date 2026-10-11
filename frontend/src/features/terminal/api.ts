import { streamJsonLines, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'
import type { CommandOptions, Target } from './connect'

export type CommandEvent = Schemas['CommandEvent']

// runs without a terminal and streams what it writes, ending with how it exited
export function runCommand(
  scope: Scope,
  service: string,
  target: Target,
  command: string[],
  options: CommandOptions,
  onEvent: (event: CommandEvent) => void,
  signal: AbortSignal,
) {
  const body =
    target === 'exec'
      ? ({ command, machine: options.machine || null } satisfies Schemas['ExecRequest'])
      : ({ command, skip_entrypoint: options.skipEntrypoint ?? false } satisfies Schemas['RunRequest'])
  return streamJsonLines<CommandEvent>(servicePath(scope, service, `/${target}`), onEvent, signal, {
    method: 'POST',
    body: JSON.stringify(body),
    contentType: 'application/json',
  })
}
