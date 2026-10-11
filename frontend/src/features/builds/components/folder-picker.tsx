import { FolderOpen } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { hintClass, labelClass } from '@/components/ui/control-styles'
import { plural } from '@/lib/format'
import { readPicked, type PickedFolder } from '../pack'

type FolderPickerProps = { folder: PickedFolder | null; onFolder: (folder: PickedFolder | null) => void }

// the build context; the Dockerfile and build arguments are fields of the form around it
export function FolderPicker({ folder, onFolder }: FolderPickerProps) {
  const input = useRef<HTMLInputElement>(null)
  useEffect(() => {
    // not in React's types, but every current browser supports picking a whole folder
    input.current?.setAttribute('webkitdirectory', '')
  }, [])

  return (
    <div className="flex flex-col gap-1.5">
      <span className={labelClass}>Source folder</span>
      <label className="flex cursor-pointer items-center gap-3 border border-dashed border-line-strong bg-sunken px-4 py-3.5 transition-colors focus-within:outline focus-within:outline-1 focus-within:outline-offset-2 focus-within:outline-fg hover:border-fg-muted">
        <FolderOpen aria-hidden className="size-4 shrink-0 text-fg-muted" />
        <span className="min-w-0 text-sm">
          {folder ? (
            <>
              <span className="block truncate font-medium text-fg">{folder.root || 'Folder'}</span>
              <span className="block font-mono text-2xs text-fg-faint">{plural(folder.files.length, 'file')} · choose another</span>
            </>
          ) : (
            <span className="text-fg-muted">Choose the folder with your Dockerfile</span>
          )}
        </span>
        <input
          ref={input}
          type="file"
          multiple
          className="sr-only"
          onChange={(event) => onFolder(event.target.files?.length ? readPicked(event.target.files) : null)}
        />
      </label>
      <p className={hintClass}>Packed in your browser, leaving out what .dockerignore excludes, then built on the server.</p>
    </div>
  )
}
