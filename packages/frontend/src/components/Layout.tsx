import { Outlet } from 'react-router-dom'
import { useEffect, useState } from 'react'
import { NavBar } from './NavBar'
import { EvmWalletModal, TerraWalletModal } from './wallet'
import { useUIStore } from '../stores/ui'
import { useWalletStore } from '../stores/wallet'

type ThemeMode = 'dark' | 'light'

function getSystemTheme(): ThemeMode {
  if (typeof window === 'undefined') return 'dark'
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'
}

function getInitialTheme(): ThemeMode {
  const stored = window.localStorage.getItem('cl8y-theme')
  if (stored === 'dark' || stored === 'light') return stored
  if (stored === 'ocean') return 'dark'
  if (stored === 'skyblue') return 'light'
  return getSystemTheme()
}

export function Layout() {
  const { showEvmWalletModal, setShowEvmWalletModal } = useUIStore()
  const { showWalletModal, setShowWalletModal } = useWalletStore()
  const [theme, setTheme] = useState<ThemeMode>(getInitialTheme)

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
  }, [theme])

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: light)')
    const handler = () => {
      if (!window.localStorage.getItem('cl8y-theme')) {
        setTheme(media.matches ? 'light' : 'dark')
      }
    }
    media.addEventListener('change', handler)
    return () => media.removeEventListener('change', handler)
  }, [])

  const setThemeAndPersist = (mode: ThemeMode) => {
    setTheme(mode)
    window.localStorage.setItem('cl8y-theme', mode)
  }

  return (
    <div className="min-h-screen overflow-x-hidden">
      <header className="sticky top-0 z-30 border-b-2 border-white/40 bg-black/90 overflow-x-clip">
        <NavBar />
      </header>

      <main className="relative max-w-5xl mx-auto px-4 pt-3 pb-6 md:pt-4 md:pb-8">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 top-2 mx-auto h-[520px] max-w-3xl rounded-[40px] theme-hero-glow blur-3xl"
        />
        <div className="relative z-10">
          <Outlet />
        </div>
      </main>

      <footer className="border-t-2 border-white/25 py-6 text-slate-300 text-xs md:text-sm uppercase tracking-wider">
        <div className="mx-auto max-w-5xl px-4 flex flex-col gap-3 items-center justify-center md:flex-row md:justify-between">
          <p>CL8Y Bridge · Cross-chain transfers between any supported chains</p>
          <div className="flex items-center gap-2" role="group" aria-label="Theme">
            <div className="inline-flex border border-white/50 bg-black/60 p-0.5 rounded-sm">
              <button
                type="button"
                aria-pressed={theme === 'dark'}
                aria-label="Dark theme"
                className={`px-2.5 py-1 text-[11px] md:text-xs uppercase tracking-wider transition-colors ${
                  theme === 'dark' ? 'bg-white/20 text-inherit' : 'text-slate-400 hover:text-slate-300'
                }`}
                onClick={() => setThemeAndPersist('dark')}
              >
                Dark
              </button>
              <button
                type="button"
                aria-pressed={theme === 'light'}
                aria-label="Light theme"
                className={`px-2.5 py-1 text-[11px] md:text-xs uppercase tracking-wider transition-colors ${
                  theme === 'light' ? 'bg-white/20 text-inherit' : 'text-slate-400 hover:text-slate-300'
                }`}
                onClick={() => setThemeAndPersist('light')}
              >
                Light
              </button>
            </div>
          </div>
        </div>
      </footer>

      <EvmWalletModal isOpen={showEvmWalletModal} onClose={() => setShowEvmWalletModal(false)} />
      <TerraWalletModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </div>
  )
}
