const WIDTH = 64
const HEIGHT = 16

// a line of values from 0 to 1, oldest on the left
export function Sparkline({ values, label, className = '' }: { values: number[]; label: string; className?: string }) {
  const step = WIDTH / Math.max(1, values.length - 1)
  const points = values.map((value, index) => `${(index * step).toFixed(1)},${(HEIGHT - 1 - Math.min(1, Math.max(0, value)) * (HEIGHT - 2)).toFixed(1)}`)
  return (
    <svg role="img" aria-label={label} viewBox={`0 0 ${WIDTH} ${HEIGHT}`} width={WIDTH} height={HEIGHT} className={`shrink-0 overflow-visible ${className}`}>
      <title>{label}</title>
      <line x1={0} x2={WIDTH} y1={HEIGHT - 0.5} y2={HEIGHT - 0.5} className="stroke-line" strokeWidth={1} />
      <polyline points={points.join(' ')} fill="none" stroke="currentColor" strokeWidth={1.25} strokeLinejoin="round" vectorEffect="non-scaling-stroke" />
    </svg>
  )
}
