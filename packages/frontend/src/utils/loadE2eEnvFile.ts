/**
 * Load `export KEY=value` or `KEY=value` lines from `.env.e2e.local` into `process.env`.
 * Used by Vitest globalSetup before chain health checks (Terra LCD port alignment with QA host).
 */
import { readFileSync, existsSync } from 'node:fs'

/** `export KEY=value` or `KEY=value` (Vitest setup file uses the latter; write-qa-env-e2e uses export). */
const ENV_LINE = /^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$/

function stripQuotes(val: string): string {
  const t = val.trim()
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
    return t.slice(1, -1)
  }
  return t
}

/**
 * Non-empty values from the file override `process.env` (file is source of truth for local E2E).
 */
export function loadE2eEnvFile(path: string): void {
  if (!existsSync(path)) return
  const text = readFileSync(path, 'utf8')
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const m = trimmed.match(ENV_LINE)
    if (!m) continue
    const key = m[1]!
    const value = stripQuotes(m[2]!)
    if (value === '') continue
    process.env[key] = value
  }
}
