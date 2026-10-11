export function Avatar({ name, size = 'md' }: { name: string; size?: 'sm' | 'md' }) {
  return (
    <span
      aria-hidden
      className={`grid shrink-0 place-items-center border border-line-strong bg-raised font-mono font-semibold text-fg uppercase ${size === 'sm' ? 'size-6 text-2xs' : 'size-8 text-xs'}`}
    >
      {name.charAt(0)}
    </span>
  )
}
