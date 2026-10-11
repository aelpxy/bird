import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Monitor } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { ErrorState } from '@/components/ui/error-state'
import { Skeleton } from '@/components/ui/skeleton'
import { dateTime, timeAgo } from '@/lib/format'
import { notifySuccess } from '@/lib/toast'
import { endSession, sessionsQuery } from '@/features/account/api'
import { describeAgent } from '@/features/account/user-agent'

export function SessionsList({ user }: { user: string }) {
  const queryClient = useQueryClient()
  const sessions = useQuery(sessionsQuery(user))

  if (sessions.isPending) return <Skeleton className="h-40 w-full" />
  if (sessions.isError) return <ErrorState error={sessions.error} onRetry={() => void sessions.refetch()} />

  return (
    <ul className="divide-y divide-line border border-line bg-surface">
      {sessions.data.map((session) => {
        const device = describeAgent(session.agent)
        return (
          <li key={session.id} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
            <Monitor aria-hidden className="size-4 shrink-0 text-fg-faint" />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-fg" title={session.agent ?? undefined}>
                  {device}
                </span>
                {session.current && <Badge tone="ok">This browser</Badge>}
              </div>
              <p className="mt-1 font-mono text-2xs text-fg-faint">
                {session.address ?? 'Unknown address'} · signed in {timeAgo(session.created_at)} · last used{' '}
                {timeAgo(session.last_used_at)} · expires <span title={dateTime(session.expires_at)}>{timeAgo(session.expires_at)}</span>
              </p>
            </div>
            {!session.current && (
              <ConfirmDialog
                trigger={
                  <Button variant="secondary" size="sm">
                    Sign out
                  </Button>
                }
                title={`Sign out ${device}?`}
                description="That browser or device has to sign in again."
                confirmLabel="Sign out"
                onConfirm={async () => {
                  await endSession(user, session.id)
                  notifySuccess(`Signed out ${device}`)
                  await queryClient.invalidateQueries({ queryKey: sessionsQuery(user).queryKey })
                }}
              />
            )}
          </li>
        )
      })}
    </ul>
  )
}
