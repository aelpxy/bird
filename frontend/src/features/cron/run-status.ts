import type { Tone } from '@/components/ui/badge'
import type { CronRun } from './api'

export const runStatuses: Record<CronRun['status'], { label: string; tone: Tone }> = {
  running: { label: 'Running', tone: 'info' },
  succeeded: { label: 'Succeeded', tone: 'ok' },
  failed: { label: 'Failed', tone: 'bad' },
  timed_out: { label: 'Timed out', tone: 'bad' },
  interrupted: { label: 'Interrupted', tone: 'warn' },
  skipped: { label: 'Skipped', tone: 'neutral' },
}
