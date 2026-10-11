// only paths on this site, so a crafted link cannot send someone elsewhere after signing in
export function safeRedirect(target: unknown): string {
  if (typeof target !== 'string') return '/'
  const isLocalPath = target.startsWith('/') && !target.startsWith('//') && !target.startsWith('/\\')
  if (!isLocalPath || target.startsWith('/login')) return '/'
  return target
}
