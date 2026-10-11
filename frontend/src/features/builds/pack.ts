import { DockerIgnore } from './dockerignore'
import { tarBlob, type TarEntry } from './tar'

// birdd's limit on a build context
export const MAX_CONTEXT_BYTES = 512 * 1024 * 1024
const IGNORE_FILE = '.dockerignore'

export type PickedFolder = { root: string; files: { path: string; file: File }[] }

// files from a folder picker carry "root/sub/file" paths; the build context starts below root
export function readPicked(list: FileList): PickedFolder {
  const files = Array.from(list).map((file) => {
    const relative = file.webkitRelativePath || file.name
    const slash = relative.indexOf('/')
    return { path: slash === -1 ? relative : relative.slice(slash + 1), file }
  })
  const first = list[0]?.webkitRelativePath ?? ''
  return { root: first.split('/')[0] ?? '', files }
}

export type Packed = { archive: Blob; files: number; excluded: number }

// a gzipped tar of the context without what .dockerignore excludes, like the CLI sends
export async function pack(folder: PickedFolder, dockerfile: string): Promise<Packed> {
  if (!folder.files.some((entry) => entry.path === dockerfile)) {
    throw new Error(`There is no ${dockerfile} in ${folder.root || 'that folder'}.`)
  }
  const ignoreFile = folder.files.find((entry) => entry.path === IGNORE_FILE)
  const ignore = new DockerIgnore(ignoreFile ? await ignoreFile.file.text() : '')
  const always = new Set([dockerfile, IGNORE_FILE])
  const kept = folder.files
    .filter((entry) => always.has(entry.path) || !ignore.excludes(entry.path))
    .sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))
  const entries: TarEntry[] = await Promise.all(
    kept.map(async (entry) => ({ path: entry.path, file: entry.file, executable: await isScript(entry.file) })),
  )
  const archive = await new Response(tarBlob(entries).stream().pipeThrough(new CompressionStream('gzip'))).blob()
  if (archive.size > MAX_CONTEXT_BYTES) {
    throw new Error('The folder is over 512 MB even compressed. Exclude more with .dockerignore.')
  }
  return { archive, files: kept.length, excluded: folder.files.length - kept.length }
}

// browsers hide the executable bit, so files starting with #! are taken to be scripts
async function isScript(file: File): Promise<boolean> {
  if (file.size < 2) return false
  const start = new Uint8Array(await file.slice(0, 2).arrayBuffer())
  return start[0] === 0x23 && start[1] === 0x21
}
