import { useRef, useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import { deploy } from '@/features/services/api'
import type { Progress } from './components/build-progress'
import { buildFromSource } from './deploy-from-source'
import type { PickedFolder } from './pack'

type BuildOptions = { dockerfile: string; args: Record<string, string> }

// the folder choice and live progress for a dialog that deploys from source
export function useSourceDeploy(scope: Scope) {
  const [folder, setFolder] = useState<PickedFolder | null>(null)
  const [progress, setProgress] = useState<Progress | null>(null)
  const controller = useRef<AbortController | null>(null)

  // builds, then hands the image to `deployImage`, keeping the progress up the whole way
  async function buildAndDeploy<T>(name: string, options: BuildOptions, deployImage: (image: string) => Promise<T>): Promise<T> {
    controller.current = new AbortController()
    setProgress({ phase: 'packing', lines: [] })
    try {
      const image = await buildFromSource({
        scope,
        name,
        folder,
        dockerfile: options.dockerfile.trim(),
        args: options.args,
        signal: controller.current.signal,
        onPhase: (phase, detail) => setProgress((current) => ({ lines: current?.lines ?? [], phase, detail })),
        onLine: (line) => setProgress((current) => current && { ...current, lines: [...current.lines, line].slice(-500) }),
      })
      const deployed = await deployImage(image)
      setProgress(null)
      return deployed
    } catch (error) {
      // the log stays up next to the error, since that is where a failed build says why
      setProgress((current) => current && { ...current, failed: true })
      throw error
    }
  }

  // an existing service: its new image on its port, keeping its other settings
  function run(name: string, port: number, options: BuildOptions) {
    return buildAndDeploy(name, options, (image) => deploy(scope, { name, image, port }))
  }

  return { folder, setFolder, progress, run, buildAndDeploy }
}
