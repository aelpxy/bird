import { Outlet, createFileRoute, redirect } from '@tanstack/react-router'

// only server admins manage users; others would just see birdd refuse
export const Route = createFileRoute('/_app/admin')({
  beforeLoad: ({ context }) => {
    if (context.session.role !== 'admin') throw redirect({ to: '/', replace: true })
  },
  component: Outlet,
})
