const MAX_NAME = 63

// a service name from an image, like ghcr.io/acme/web:1.4 → web, following birdd's name rule
export function nameFromImage(image: string): string {
  const path = image.trim().split('@')[0] ?? ''
  const last = path.split('/').pop() ?? ''
  const repository = last.split(':')[0] ?? ''
  const cleaned = repository
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^[^a-z]+/, '')
    .replace(/-+/g, '-')
    .replace(/-$/, '')
  return cleaned.slice(0, MAX_NAME).replace(/-$/, '')
}
