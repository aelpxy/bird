import type { Scope } from '@/lib/api/paths'
import { deploy } from '@/features/services/api'
import { build } from './api'
import { pack, type PickedFolder } from './pack'

export type SourcePhase = 'packing' | 'building' | 'deploying'

type SourceDeploy = {
  scope: Scope
  name: string
  port: number
  folder: PickedFolder | null
  dockerfile: string
  args: Record<string, string>
  onPhase: (phase: SourcePhase, detail?: string) => void
  onLine: (line: string) => void
  signal: AbortSignal
}

// pack the folder, build it on the server, then deploy the image it made
export async function deployFromSource(options: SourceDeploy) {
  const { scope, name, folder, onPhase } = options
  if (!folder || folder.files.length === 0) throw new Error('Choose the folder with your Dockerfile.')
  onPhase('packing')
  const packed = await pack(folder, options.dockerfile)
  onPhase('building', `${packed.files} files, ${(packed.archive.size / 1024 / 1024).toFixed(1)} MB compressed`)
  const image = await build(scope, name, packed.archive, { dockerfile: options.dockerfile, args: options.args }, options.onLine, options.signal)
  onPhase('deploying', image)
  return deploy(scope, { name, image, port: options.port })
}
