import { Outlet } from 'react-router-dom'
import { NavBar } from './NavBar'

export function Layout() {
  return (
    <div className="min-h-screen overflow-x-hidden">
      <header className="sticky top-0 z-30 border-b-2 border-white/40 bg-black/90 overflow-x-clip">
        <NavBar />
      </header>

      <main className="relative max-w-5xl mx-auto px-4 pt-3 pb-6 md:pt-4 md:pb-8">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-x-0 top-2 mx-auto h-[520px] max-w-3xl rounded-[40px] bg-[radial-gradient(circle,_rgba(255,255,255,0.12)_0%,_rgba(0,0,0,0)_70%)] blur-3xl"
        />
        <div className="relative z-10">
          <Outlet />
        </div>
      </main>

      <footer className="border-t-2 border-white/25 py-6 text-center text-slate-300 text-xs md:text-sm uppercase tracking-wider">
        <p className="px-4">CL8Y Bridge · Cross-chain transfers between any supported chains</p>
      </footer>
    </div>
  )
}
