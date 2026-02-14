import { Outlet } from 'react-router-dom'
import { useEffect, useState } from 'react'
import { NavBar } from './NavBar'
import { EvmWalletModal, TerraWalletModal } from './wallet'
import { useUIStore } from '../stores/ui'
import { useWalletStore } from '../stores/wallet'

type ThemeOption = 'default' | 'sunset' | 'ocean' | 'retro' | 'frost' | 'skyblue'

export function Layout() {
  const { showEvmWalletModal, setShowEvmWalletModal } = useUIStore()
  const { showWalletModal, setShowWalletModal } = useWalletStore()
  const [theme, setTheme] = useState<ThemeOption>(() => {
    const storedTheme = window.localStorage.getItem('cl8y-theme')
    if (
      storedTheme === 'sunset' ||
      storedTheme === 'ocean' ||
      storedTheme === 'retro' ||
      storedTheme === 'frost' ||
      storedTheme === 'skyblue'
    ) {
      return storedTheme
    }
    return 'default'
  })

  useEffect(() => {
    if (theme === 'default') {
      document.documentElement.removeAttribute('data-theme')
      window.localStorage.removeItem('cl8y-theme')
      return
    }

    document.documentElement.setAttribute('data-theme', theme)
    window.localStorage.setItem('cl8y-theme', theme)
  }, [theme])

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
          <label className="flex items-center gap-2">
            <span>Theme</span>
            <select
              className="border border-white/50 bg-black/60 px-2 py-1 text-[11px] md:text-xs"
              value={theme}
              onChange={(event) => setTheme(event.target.value as ThemeOption)}
            >
              <option value="default">Default</option>
              <option value="sunset">Sunset</option>
              <option value="ocean">Ocean</option>
              <option value="retro">Retro</option>
              <option value="frost">Frost</option>
              <option value="skyblue">Skyblue</option>
            </select>
          </label>
        </div>
      </footer>

      <EvmWalletModal isOpen={showEvmWalletModal} onClose={() => setShowEvmWalletModal(false)} />
      <TerraWalletModal isOpen={showWalletModal} onClose={() => setShowWalletModal(false)} />
    </div>
  )
}
