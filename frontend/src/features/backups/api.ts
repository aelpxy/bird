import { queryOptions } from '@tanstack/react-query'
import { api, type Schemas } from '@/lib/api/client'
import { scopePath, servicePath, type Scope } from '@/lib/api/paths'

export type Backup = Schemas['BackupInfo']
export type BackupSchedule = Schemas['BackupSchedule']

export const backupsQuery = (scope: Scope, service: string) =>
  queryOptions({
    queryKey: ['services', scope.project, scope.environment, service, 'backups'],
    queryFn: ({ signal }) => api<Backup[]>(servicePath(scope, service, '/backups'), { signal }),
  })

export function createBackup(scope: Scope, service: string) {
  return api<Backup>(servicePath(scope, service, '/backups'), { method: 'POST' })
}

export function removeBackup(scope: Scope, service: string, id: string) {
  return api<void>(servicePath(scope, service, `/backups/${encodeURIComponent(id)}`), { method: 'DELETE' })
}

export function restoreBackup(scope: Scope, service: string, id: string, allowImageChange: boolean) {
  const body: Schemas['RestoreRequest'] = { allow_image_change: allowImageChange }
  return api<Schemas['RestoreResponse']>(servicePath(scope, service, `/backups/${encodeURIComponent(id)}/restore`), {
    method: 'POST',
    body,
  })
}

export function setSchedule(scope: Scope, service: string, schedule: BackupSchedule) {
  return api<BackupSchedule>(servicePath(scope, service, '/backups/schedule'), { method: 'PUT', body: schedule })
}

export function clearSchedule(scope: Scope, service: string) {
  return api<void>(servicePath(scope, service, '/backups/schedule'), { method: 'DELETE' })
}

// birdd allows one hour up to thirty days between backups
export const INTERVALS = [
  { seconds: 6 * 3600, label: 'Every 6 hours' },
  { seconds: 12 * 3600, label: 'Every 12 hours' },
  { seconds: 86_400, label: 'Every day' },
  { seconds: 7 * 86_400, label: 'Every week' },
]
export const MAX_KEEP = 1000

// every backup in an environment, also of services removed since, so leftovers can be found and cleared
export const environmentBackupsQuery = (scope: Scope) =>
  queryOptions({
    queryKey: ['backups', scope.project, scope.environment],
    queryFn: ({ signal }) => api<Backup[]>(`${scopePath(scope)}/backups`, { signal }),
  })
