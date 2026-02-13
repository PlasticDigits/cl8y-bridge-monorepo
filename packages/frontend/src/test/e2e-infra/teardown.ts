/**
 * E2E Test Infrastructure Teardown
 *
 * Cleans up all test infrastructure:
 * 1. Stop and remove Docker containers and volumes
 * 2. Delete .env.e2e.local
 * 3. Kill any orphaned processes
 */

import { execSync } from 'child_process'
import { unlinkSync, existsSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'
import { stopOperator } from './operator'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ROOT_DIR = resolve(__dirname, '../../../../..')
const FRONTEND_DIR = resolve(__dirname, '../../..')
const ENV_FILE = resolve(ROOT_DIR, '.env.e2e.local')
const VITE_ENV_FILE = resolve(FRONTEND_DIR, '.env.local')

export default async function teardown(): Promise<void> {
  console.log('=== E2E Test Infrastructure Teardown ===\n')

  // 0. Stop operator first (before docker compose down)
  console.log('[teardown] Stopping operator...')
  try {
    await stopOperator()
  } catch (error) {
    console.warn('[teardown] Operator stop failed:', (error as Error).message?.slice(0, 200))
  }

  // 1. Stop Docker containers and remove volumes
  console.log('[teardown] Stopping Docker containers...')
  try {
    execSync('docker compose down -v', {
      cwd: ROOT_DIR,
      stdio: 'inherit',
      timeout: 30_000,
    })
  } catch (error) {
    console.warn('[teardown] docker compose down failed (may already be stopped):', (error as Error).message?.slice(0, 200))
  }

  // 2. Delete environment files
  if (existsSync(ENV_FILE)) {
    console.log('[teardown] Removing .env.e2e.local...')
    unlinkSync(ENV_FILE)
  }
  if (existsSync(VITE_ENV_FILE)) {
    console.log('[teardown] Removing packages/frontend/.env.local...')
    unlinkSync(VITE_ENV_FILE)
  }

  // 3. Kill any orphaned processes on test ports
  const ports = [8545, 8546, 26657, 1317, 9090, 9091, 9092, 5433]
  for (const port of ports) {
    try {
      execSync(`lsof -ti :${port} | xargs kill -9 2>/dev/null || true`, {
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'pipe'],
      })
    } catch {
      // Ignore - port may not be in use
    }
  }

  console.log('\n=== E2E Teardown Complete ===\n')
}

// Allow running directly: npx tsx src/test/e2e-infra/teardown.ts
const isMain = process.argv[1] && import.meta.url.endsWith(process.argv[1].replace(/^\//, ''))
  || process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)
if (isMain) {
  teardown().catch((err) => {
    console.error(err)
    process.exit(1)
  })
}
