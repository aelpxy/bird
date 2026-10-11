export type Item = { id: string; label: string; detail: string; go: () => unknown }

const MAX_RESULTS = 50

// every word typed must appear somewhere in the item, in any order
export function filter(items: Item[], query: string): Item[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  return items
    .filter((item) => {
      const text = `${item.label} ${item.detail}`.toLowerCase()
      return words.every((word) => text.includes(word))
    })
    .slice(0, MAX_RESULTS)
}
