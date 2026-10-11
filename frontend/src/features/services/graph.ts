type Node = { name: string; references?: string[] }

// columns for the map: services that read others first, what nothing else is read from last
export function mapColumns<T extends Node>(services: T[]): T[][] {
  const byName = new Map(services.map((service) => [service.name, service]))
  const depth = new Map<string, number>()
  // a cycle cannot be saved, but a missing guard would hang the page if one ever were
  const visiting = new Set<string>()
  function measure(service: T): number {
    const known = depth.get(service.name)
    if (known !== undefined) return known
    if (visiting.has(service.name)) return 0
    visiting.add(service.name)
    const below = (service.references ?? []).flatMap((name) => {
      const target = byName.get(name)
      return target ? [measure(target) + 1] : []
    })
    visiting.delete(service.name)
    const found = Math.max(0, ...below)
    depth.set(service.name, found)
    return found
  }
  const deepest = Math.max(0, ...services.map(measure))
  const columns: T[][] = Array.from({ length: deepest + 1 }, () => [])
  for (const service of services) columns[deepest - (depth.get(service.name) ?? 0)].push(service)
  return columns.filter((column) => column.length > 0)
}
