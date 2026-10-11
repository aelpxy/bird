import { streamJsonLines, type Schemas } from '@/lib/api/client'
import { servicePath, type Scope } from '@/lib/api/paths'

export type BuildEvent = Schemas['BuildEvent']

// streams the build's output; resolves with the image to deploy, or rejects with why it failed
export async function build(
  scope: Scope,
  service: string,
  archive: Blob,
  options: { dockerfile: string; args: Record<string, string> },
  onLine: (line: string) => void,
  signal: AbortSignal,
): Promise<string> {
  const query = new URLSearchParams({ dockerfile: options.dockerfile })
  if (Object.keys(options.args).length > 0) query.set('args', JSON.stringify(options.args))
  let image: string | undefined
  let failure: string | undefined
  await streamJsonLines<BuildEvent>(
    servicePath(scope, service, `/builds?${query}`),
    (event) => {
      if (event.type === 'log') onLine(event.line)
      else if (event.type === 'built') image = event.image
      else failure = event.error
    },
    signal,
    { method: 'POST', body: archive, contentType: 'application/x-tar' },
  )
  if (image) return image
  throw new Error(failure ? `The build failed: ${failure}` : 'The build ended without an image.')
}
