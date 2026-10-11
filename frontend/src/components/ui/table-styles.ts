// narrow screens stack each row: the first two cells share a line, the rest follow with their labels
const STACKED = 'grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-4 gap-y-1.5 px-4'

// `columns` is the wide layout, like sm:grid-cols-[1fr_8rem_6rem]
export function tableRowClass(columns: string): string {
  return `${STACKED} sm:gap-x-6 ${columns}`
}
