import { describe, expect, it } from 'vitest'
import { DockerIgnore } from './dockerignore'

// the same cases as the CLI's own tests, so both pack a folder alike
describe('DockerIgnore', () => {
  it('matches like docker', () => {
    const ignore = new DockerIgnore('# deps\nnode_modules\n/.git/\n*.log\n**/tmp\ndocs/*.md\n!docs/keep.md\n')
    for (const excluded of ['node_modules', 'node_modules/a/b.js', '.git/HEAD', 'app.log', 'a/b/tmp', 'a/b/tmp/x', 'tmp', 'docs/readme.md']) {
      expect(ignore.excludes(excluded), excluded).toBe(true)
    }
    for (const included of ['src/main.rs', 'nested/app.log', 'docs/keep.md', 'docs/sub/a.md']) {
      expect(ignore.excludes(included), included).toBe(false)
    }
  })

  it('lets exceptions bring files back', () => {
    const ignore = new DockerIgnore('vendor\n!vendor/keep\n')
    expect(ignore.excludes('vendor/other')).toBe(true)
    expect(ignore.excludes('vendor/keep')).toBe(false)
  })

  it('sends everything when a pattern is unsupported', () => {
    expect(new DockerIgnore('node_modules\n[ab].txt\n').excludes('node_modules')).toBe(false)
  })
})
