import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { inputClass, labelClass } from '@/components/ui/control-styles'
import { Panel } from '@/components/ui/panel'
import { SelectField } from '@/components/ui/select-field'
import type { Scope } from '@/lib/api/paths'
import { interval, plural } from '@/lib/format'
import { notifyError, notifySuccess } from '@/lib/toast'
import type { Service } from '@/features/services/api'
import { INTERVALS, MAX_KEEP, clearSchedule, setSchedule } from '@/features/backups/api'

export function SchedulePanel({ scope, service }: { scope: Scope; service: Service }) {
  const queryClient = useQueryClient()
  const current = service.backup_schedule
  const [every, setEvery] = useState(current?.every ?? 86_400)
  const [keep, setKeep] = useState(current?.keep ?? 7)
  const refresh = () => queryClient.invalidateQueries({ queryKey: ['services', scope.project, scope.environment, service.name] })
  const save = useMutation({
    mutationFn: () => setSchedule(scope, service.name, { every, keep }),
    onSuccess: () => notifySuccess('Backup schedule saved', `${interval(every)} apart, keeping ${plural(keep, 'copy', 'copies')}.`),
    onError: (error) => notifyError("Couldn't save the schedule", error),
    onSettled: refresh,
  })
  const stop = useMutation({
    mutationFn: () => clearSchedule(scope, service.name),
    onSuccess: () => notifySuccess('Scheduled backups are off'),
    onError: (error) => notifyError("Couldn't turn off the schedule", error),
    onSettled: refresh,
  })
  // a schedule set in bird.toml may use an interval this list does not offer
  const options = INTERVALS.some((option) => option.seconds === every)
    ? INTERVALS
    : [...INTERVALS, { seconds: every, label: `Every ${interval(every)}` }]

  return (
    <Panel className="flex flex-col gap-4">
      <p className="text-sm text-fg">
        {current
          ? `Backed up every ${interval(current.every)}, keeping the newest ${plural(current.keep, 'copy', 'copies')}.`
          : 'No schedule. Backups only happen when you start one.'}
      </p>
      <div className="grid gap-3 sm:grid-cols-[1fr_9rem_auto] sm:items-end">
        <SelectField
          label="How often"
          value={every}
          options={options.map((option) => ({ value: option.seconds, label: option.label }))}
          onChange={setEvery}
        />
        <label className="flex flex-col gap-1.5">
          <span className={labelClass}>Copies to keep</span>
          <input
            type="number"
            inputMode="numeric"
            min={1}
            max={MAX_KEEP}
            value={keep}
            onChange={(event) => setKeep(Math.min(MAX_KEEP, Math.max(1, Number(event.target.value) || 1)))}
            className={inputClass}
          />
        </label>
        <div className="flex gap-2">
          <Button variant="secondary" loading={save.isPending} onClick={() => save.mutate()}>
            {current ? 'Update' : 'Turn on'}
          </Button>
          {current && (
            <Button variant="ghost" loading={stop.isPending} onClick={() => stop.mutate()}>
              Turn off
            </Button>
          )}
        </div>
      </div>
    </Panel>
  )
}
