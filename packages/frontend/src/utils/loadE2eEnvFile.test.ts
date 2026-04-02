import { writeFileSync, unlinkSync, mkdtempSync } from 'node:fs'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { afterEach, describe, expect, it } from 'vitest'
import { loadE2eEnvFile } from './loadE2eEnvFile'
import { terraLcdUrl } from '../test/e2e-infra/health'

describe('loadE2eEnvFile', () => {
  let dir: string
  let prevTerra: string | undefined

  afterEach(() => {
    if (prevTerra === undefined) delete process.env.TERRA_LCD_URL
    else process.env.TERRA_LCD_URL = prevTerra
    delete process.env.VITE_FOO
    try {
      if (dir) unlinkSync(join(dir, 'env'))
    } catch {
      /* ignore */
    }
  })

  it('parses export and plain KEY=value; overrides process.env', () => {
    prevTerra = process.env.TERRA_LCD_URL
    dir = mkdtempSync(join(tmpdir(), 'e2e-env-'))
    const p = join(dir, 'env')
    writeFileSync(
      p,
      [
        '# comment',
        'export TERRA_LCD_URL=http://127.0.0.1:1318',
        'VITE_FOO=bar',
        'export EMPTY=',
      ].join('\n'),
      'utf8'
    )
    process.env.TERRA_LCD_URL = 'http://localhost:1317'
    loadE2eEnvFile(p)
    expect(process.env.TERRA_LCD_URL).toBe('http://127.0.0.1:1318')
    expect(process.env.VITE_FOO).toBe('bar')
    expect(terraLcdUrl()).toBe('http://127.0.0.1:1318')
  })

  it('strips double quotes on values', () => {
    dir = mkdtempSync(join(tmpdir(), 'e2e-env-'))
    const p = join(dir, 'env')
    writeFileSync(p, 'export TERRA_LCD_URL="http://127.0.0.1:9999"\n', 'utf8')
    loadE2eEnvFile(p)
    expect(process.env.TERRA_LCD_URL).toBe('http://127.0.0.1:9999')
  })
})
