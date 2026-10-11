import { useRef, useState } from 'react'
import type { Scope } from '@/lib/api/paths'
import type { Progress } from './components/build-progress'
import { deployFromSource } from './deploy-from-source'
import type { PickedFolder } from './pack'

// the folder choice and live progress for a dialog that deploys from source
export function useSourceDeploy(scope: Scope) {
  const [folder, setFolder] = useState<PickedFolder | null>(null)
  const [progress, setProgress] = useState<Progress | null>(null)
  const controller = useRef<AbortController | null>(null)

  async function run(name: string, port: number, values: { dockerfile: string; args: Record<string, string> }) {
    controller.current = new AbortController()
    setProgress({ phase: 'packing', lines: [] })
    try {
      const deployed = await deployFromSource({
        scope,
        name,
        port,
        folder,
        dockerfile: values.dockerfile.trim(),
        args: values.args,
        signal: controller.current.signal,
        onPhase: (phase, detail) => setProgress((current) => ({ lines: current?.lines ?? [], phase, detail })),
        onLine: (line) => setProgress((current) => current && { ...current, lines: [...current.lines, line].slice(-500) }),
      })
      setProgress(null)
      return deployed
    } catch (error) {
      // the log stays up next to the error, since that is where a failed build says why
      setProgress((current) => current && { ...current, failed: true })
      throw error
    }
  }

  return { folder, setFolder, progress, run }
}
