import { Outlet } from 'react-router-dom'
import { NavBar } from './NavBar'

export function Layout() {
  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-30 border-b-2 border-white/40 bg-black/90">
        <NavBar />
      </header>

      <main className="max-w-5xl mx-auto px-4 py-10 md:py-12">
        <Outlet />
      </main>

      <footer className="border-t-2 border-white/25 py-6 text-center text-slate-300 text-xs md:text-sm uppercase tracking-wider">
        <p className="px-4">CL8Y Bridge · Cross-chain transfers between Terra Classic and EVM chains</p>
      </footer>
    </div>
  )
}
