import bird56 from '@/assets/bird-56.png'
import bird80 from '@/assets/bird-80.png'
import bird160 from '@/assets/bird-160.png'

// the pixel-art bird from the readme, pre-scaled so it stays crisp at each size
export function Logo({ size = 'md' }: { size?: 'md' | 'lg' }) {
  if (size === 'lg') {
    return <img src={bird80} srcSet={`${bird80} 1x, ${bird160} 2x`} width={40} height={40} alt="bird" className="size-10" />
  }
  return (
    <span className="flex items-center gap-2 pr-1">
      <img src={bird56} srcSet={`${bird56} 2x`} width={28} height={28} alt="" className="size-7" />
      <span className="font-mono text-ui leading-none font-semibold tracking-tight text-fg">bird</span>
    </span>
  )
}
