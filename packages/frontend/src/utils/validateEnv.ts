/**
 * Fail-fast environment variable validation for production builds.
 * Prevents the app from silently falling back to zero-address contracts.
 */

const REQUIRED_ENV_VARS = [
  'VITE_NETWORK',
  'VITE_TERRA_BRIDGE_ADDRESS',
  'VITE_EVM_BRIDGE_ADDRESS',
  'VITE_EVM_ROUTER_ADDRESS',
  'VITE_BRIDGE_TOKEN_ADDRESS',
  'VITE_LOCK_UNLOCK_ADDRESS',
  'VITE_EVM_RPC_URL',
  'VITE_TERRA_LCD_URL',
  'VITE_TERRA_RPC_URL',
  'VITE_WC_PROJECT_ID',
] as const

export function validateEnv(): void {
  if (import.meta.env.MODE !== 'production') return

  const missing = REQUIRED_ENV_VARS.filter(
    (key) => !import.meta.env[key]
  )

  if (missing.length > 0) {
    const msg = `[CL8Y Bridge] Missing required environment variables:\n${missing.map((k) => `  - ${k}`).join('\n')}\n\nSet these in .env.production before building.`

    document.getElementById('root')!.innerHTML = `
      <div style="display:flex;align-items:center;justify-content:center;min-height:100vh;background:#0f172a;color:#f87171;font-family:monospace;padding:2rem;">
        <pre style="white-space:pre-wrap;max-width:640px;">${msg}</pre>
      </div>
    `

    throw new Error(msg)
  }
}
