import { servicePath, type Scope } from '@/lib/api/paths'

export type Target = 'exec' | 'run'

export type CommandOptions = {
  // a running machine, or the end of its id; left out, the first one
  machine?: string
  // run the program itself instead of handing it to the image's entrypoint
  skipEntrypoint?: boolean
}

// the browser sends the session cookie and its Origin with the upgrade, which is how birdd lets it in
export function socketUrl(
  scope: Scope,
  service: string,
  target: Target,
  kind: 'tty' | 'pipe',
  command: string[],
  options: CommandOptions & { cols?: number; rows?: number },
): string {
  const query = new URLSearchParams({ command: JSON.stringify(command) })
  if (options.cols !== undefined) query.set('cols', String(options.cols))
  if (options.rows !== undefined) query.set('rows', String(options.rows))
  if (target === 'exec' && options.machine) query.set('machine', options.machine)
  if (target === 'run' && options.skipEntrypoint) query.set('skip_entrypoint', 'true')
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${protocol}//${window.location.host}${servicePath(scope, service, `/${target}/${kind}`)}?${query}`
}
