/**
 * Resolve the LocalTerra Docker container for `docker exec` / `docker cp`.
 * Compose project names vary by checkout path; `register-tokens` and `deploy-terra` must agree.
 */
import { execSync } from 'child_process'
import { dirname, resolve } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))

/** Repo root when this file lives in packages/frontend/src/test/e2e-infra/ */
export const E2E_INFRA_DEFAULT_REPO_ROOT = resolve(__dirname, '../../../../..')

export const LEGACY_LOCALTERRA_CONTAINER_NAME = 'cl8y-bridge-monorepo-localterra-1'

export function resolveLocalterraDockerExecTarget(repoRoot: string = E2E_INFRA_DEFAULT_REPO_ROOT): string {
  const explicit = process.env.LOCALTERRA_DOCKER_CONTAINER?.trim()
  if (explicit) return explicit
  try {
    const out = execSync('docker compose ps -q localterra', {
      cwd: repoRoot,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env, FOUNDRY_DISABLE_NIGHTLY_WARNING: '1' },
    }).trim()
    const first = out.split(/\r?\n/).find((line) => line.length > 0)
    if (first) return first
  } catch {
    /* fall through */
  }
  console.warn(
    `[e2e-infra] Could not resolve localterra via "docker compose ps -q localterra" from ${repoRoot}; using "${LEGACY_LOCALTERRA_CONTAINER_NAME}". Set LOCALTERRA_DOCKER_CONTAINER if exec fails.`,
  )
  return LEGACY_LOCALTERRA_CONTAINER_NAME
}
