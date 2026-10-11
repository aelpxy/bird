import { describe, expect, it } from 'vitest'
import { splitCommand } from './split-command'

describe('splitCommand', () => {
  it('splits on any whitespace', () => {
    expect(splitCommand('  psql   -U\tpostgres ')).toEqual({ ok: true, args: ['psql', '-U', 'postgres'] })
  })

  it('keeps quoted text together, including empty arguments', () => {
    expect(splitCommand(`psql -c "select 1, 'a'" ''`)).toEqual({
      ok: true,
      args: ['psql', '-c', "select 1, 'a'", ''],
    })
    expect(splitCommand(`echo 'say "hi"'`)).toEqual({ ok: true, args: ['echo', 'say "hi"'] })
  })

  it('takes a backslash literally inside single quotes only', () => {
    expect(splitCommand(String.raw`echo a\ b "c\"d" 'e\f'`)).toEqual({
      ok: true,
      args: ['echo', 'a b', 'c"d', String.raw`e\f`],
    })
  })

  it('joins quoted and bare parts of one argument', () => {
    expect(splitCommand(`--name="my app"x`)).toEqual({ ok: true, args: ['--name=my appx'] })
  })

  it('reports what is wrong', () => {
    expect(splitCommand('echo "open')).toEqual({ ok: false, error: 'A double quote is never closed.' })
    expect(splitCommand("echo 'open")).toEqual({ ok: false, error: 'A single quote is never closed.' })
    expect(splitCommand('echo \\')).toEqual({ ok: false, error: 'The command ends with a lone backslash.' })
    expect(splitCommand('   ')).toEqual({ ok: false, error: 'Enter a command to run.' })
  })
})
