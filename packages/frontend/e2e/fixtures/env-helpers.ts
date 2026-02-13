/**
 * Environment helpers for E2E verification tests.
 * Loads .env.e2e.local and provides configurable URLs with sensible defaults.
 */

import { readFileSync, existsSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)
const ROOT_DIR = resolve(__dirname, '../../../../')
const ENV_FILE = resolve(ROOT_DIR, '.env.e2e.local')

export { ROOT_DIR, ENV_FILE }

/**
 * Load env vars from .env.e2e.local (monorepo root).
 */
export function loadEnv(): Record<string, string> {
  const vars: Record<string, string> = {}
  if (!existsSync(ENV_FILE)) return vars
  const content = readFileSync(ENV_FILE, 'utf8')
  for (const line of content.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const eq = trimmed.indexOf('=')
    if (eq > 0) vars[trimmed.slice(0, eq)] = trimmed.slice(eq + 1)
  }
  return vars
}

/** Anvil RPC URL (default: http://localhost:8545). Override via ANVIL_RPC_URL in env. */
export function getAnvilRpcUrl(env?: Record<string, string>): string {
  return (env ?? loadEnv())['ANVIL_RPC_URL'] || 'http://localhost:8545'
}

/** Anvil1 RPC URL (default: http://localhost:8546). Override via ANVIL1_RPC_URL in env. */
export function getAnvil1RpcUrl(env?: Record<string, string>): string {
  return (env ?? loadEnv())['ANVIL1_RPC_URL'] || 'http://localhost:8546'
}

/** Terra LCD URL (default: http://localhost:1317). Override via TERRA_LCD_URL in env. */
export function getTerraLcdUrl(env?: Record<string, string>): string {
  return (env ?? loadEnv())['TERRA_LCD_URL'] || 'http://localhost:1317'
}
