import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { UserMinus } from 'lucide-react'
import { z } from 'zod'
import { useAppForm } from '@/components/form/app-form'
import { Avatar } from '@/components/ui/avatar'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { ErrorState } from '@/components/ui/error-state'
import { Eyebrow } from '@/components/ui/eyebrow'
import { IconButton } from '@/components/ui/icon-button'
import { SelectField } from '@/components/ui/select-field'
import { Skeleton } from '@/components/ui/skeleton'
import { notifyError, notifySuccess } from '@/lib/toast'
import { name } from '@/lib/validation'
import type { Session } from '@/features/auth/api'
import { usersQuery } from '@/features/users/api'
import { ORG_ROLES, membersQuery, removeMember, setMember, type OrgRole } from '@/features/orgs/api'

const roleOptions = ORG_ROLES.map((role) => ({ value: role.value, label: role.label }))

export function MembersPanel({ org, session }: { org: string; session: Session }) {
  const queryClient = useQueryClient()
  const members = useQuery(membersQuery(org))
  const refresh = () => queryClient.invalidateQueries({ queryKey: membersQuery(org).queryKey })
  const change = useMutation({
    mutationFn: ({ user, role }: { user: string; role: OrgRole }) => setMember(org, user, role),
    onSuccess: (_result, { user, role }) => notifySuccess(`${user} is now ${role} of ${org}`),
    onError: (error, { user }) => notifyError(`Couldn't change ${user}'s role`, error),
    onSettled: refresh,
  })

  if (members.isPending) return <Skeleton className="h-40 w-full" />
  if (members.isError) return <ErrorState error={members.error} onRetry={() => void members.refetch()} />

  return (
    <div className="flex flex-col gap-6">
      {members.data.length === 0 ? (
        <p className="border border-dashed border-line-strong px-4 py-6 text-center text-sm text-fg-muted">
          No members yet. Server admins can still manage it; add someone below.
        </p>
      ) : (
      <ul className="divide-y divide-line border border-line bg-surface">
        {members.data.map((member) => (
          <li key={member.user} className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
            <Avatar name={member.user} />
            <span className="min-w-0 flex-1 truncate font-medium text-fg">
              {member.user}
              {member.user === session.name && <span className="ml-2 text-xs font-normal text-fg-faint">(you)</span>}
            </span>
            <div className="w-36">
              <SelectField
                label={`Role of ${member.user}`}
                hideLabel
                value={member.role}
                options={roleOptions}
                disabled={change.isPending}
                onChange={(role) => change.mutate({ user: member.user, role })}
              />
            </div>
            <ConfirmDialog
              trigger={
                <IconButton label={`Remove ${member.user} from ${org}`} tone="danger">
                  <UserMinus aria-hidden className="size-4" />
                </IconButton>
              }
              title={member.user === session.name ? `Leave ${org}?` : `Remove ${member.user} from ${org}?`}
              description="They lose access to the org's projects. birdd keeps at least one owner."
              confirmLabel={member.user === session.name ? 'Leave org' : 'Remove member'}
              tone="danger"
              onConfirm={async () => {
                await removeMember(org, member.user)
                notifySuccess(`Removed ${member.user} from ${org}`)
                await refresh()
              }}
            />
          </li>
        ))}
      </ul>
      )}
      <AddMember org={org} existing={members.data.map((member) => member.user)} isAdmin={session.role === 'admin'} onAdded={refresh} />
    </div>
  )
}

type AddMemberProps = { org: string; existing: string[]; isAdmin: boolean; onAdded: () => Promise<unknown> }

const addSchema = z.object({ user: name('Enter a username.'), role: z.enum(['owner', 'admin', 'member']) })

function AddMember({ org, existing, isAdmin, onAdded }: AddMemberProps) {
  // only admins can list users; everyone else types the name
  const users = useQuery({ ...usersQuery, enabled: isAdmin })
  const candidates = (users.data ?? []).map((user) => user.name).filter((user) => !existing.includes(user))
  const add = useMutation({
    mutationFn: ({ user, role }: z.output<typeof addSchema>) => setMember(org, user, role),
    onSuccess: async (_result, { user }) => {
      notifySuccess(`Added ${user} to ${org}`)
      form.reset()
      await onAdded()
    },
    onError: (error, { user }) => notifyError(`Couldn't add ${user}`, error),
  })
  const form = useAppForm({
    defaultValues: { user: '', role: 'member' as OrgRole },
    validators: { onSubmit: addSchema },
    onSubmit: ({ value }) => add.mutateAsync(addSchema.parse({ ...value, user: value.user || candidates[0] || '' })).catch(() => undefined),
  })
  const noneLeft = isAdmin && candidates.length === 0

  return (
    <form.AppForm>
      <form.FormElement className="flex flex-col gap-3 border border-line bg-surface p-4">
        <Eyebrow>Add a member</Eyebrow>
        <div className="grid gap-3 sm:grid-cols-[1fr_11rem_auto] sm:items-start">
          <form.AppField name="user">
            {(field) =>
              isAdmin ? (
                <field.SelectField label="User" options={candidates.map((user) => ({ value: user, label: user }))} disabled={noneLeft} />
              ) : (
                <field.TextField label="Username" autoComplete="off" autoCapitalize="none" spellCheck={false} />
              )
            }
          </form.AppField>
          <form.AppField name="role">{(field) => <field.SelectField label="Role" options={roleOptions} />}</form.AppField>
          <div className="sm:mt-[1.6rem]">
            <form.SubmitButton variant="secondary" disabled={noneLeft}>
              Add
            </form.SubmitButton>
          </div>
        </div>
        {noneLeft && <p className="text-xs text-fg-faint">Every user is already a member. Create more under Users.</p>}
      </form.FormElement>
    </form.AppForm>
  )
}
