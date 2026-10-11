import { ArrowRight, Box, Database, FolderCode } from 'lucide-react'
import { focusRing } from '@/components/ui/control-styles'
import type { Source } from '../../new-service/schema'

const sources = [
  { value: 'image', title: 'Container image', description: 'Run any public or private image.', example: 'ghcr.io/acme/web:1.4', icon: Box },
  { value: 'source', title: 'Source folder', description: 'Build a folder with a Dockerfile on the server.', example: './my-app/Dockerfile', icon: FolderCode },
  { value: 'template', title: 'Database or search', description: 'A ready-made database, cache or search engine with a volume.', example: 'postgres · mysql · clickhouse · meilisearch · dragonfly', icon: Database },
] as const

// the first step: what the service is made from
export function SourcePicker({ onPick }: { onPick: (source: Source) => void }) {
  return (
    <ul className="flex flex-col gap-2">
      {sources.map((source) => (
        <li key={source.value}>
          <button
            type="button"
            onClick={() => onPick(source.value)}
            className={`pressable group flex w-full items-center gap-4 border border-line bg-surface p-4 text-left hover:border-line-strong hover:bg-raised ${focusRing}`}
          >
            <span className="grid size-10 shrink-0 place-items-center border border-line-strong bg-sunken text-fg">
              <source.icon aria-hidden className="size-4" />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block text-sm font-medium text-fg">{source.title}</span>
              <span className="mt-0.5 block text-xs text-fg-muted">{source.description}</span>
              <span className="mt-1.5 block truncate font-mono text-2xs text-fg-faint">{source.example}</span>
            </span>
            <ArrowRight aria-hidden className="size-4 shrink-0 text-fg-faint transition-transform group-hover:translate-x-0.5 group-hover:text-fg" />
          </button>
        </li>
      ))}
    </ul>
  )
}
