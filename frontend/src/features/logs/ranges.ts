// how far back the viewer reads before following; live reads the last few hundred lines
export const RANGES = [
  { value: 'live', label: 'Live', minutes: undefined },
  { value: '15m', label: '15m', minutes: 15 },
  { value: '1h', label: '1h', minutes: 60 },
  { value: '6h', label: '6h', minutes: 360 },
  { value: '24h', label: '24h', minutes: 1440 },
] as const

export type LogRange = (typeof RANGES)[number]['value']

export function minutesOf(range: LogRange): number | undefined {
  return RANGES.find((candidate) => candidate.value === range)?.minutes
}

export function parseRange(value: unknown): LogRange | undefined {
  return RANGES.find((candidate) => candidate.value === value)?.value
}
