/**
 * Shared class strings for the app shell (`Layout`).
 *
 * GL-126: On viewports below the `md` breakpoint, the sticky header + NavBar stack is tall
 * and decor (hero glow blur) can read as overlapping top chrome. Extra main padding and a
 * lower glow anchor on small screens keep primary content clearly separated from the nav card.
 *
 * @see docs/FRONTEND_BRIDGE_INVARIANTS.md — INV-MOB1
 */
export const LAYOUT_MAIN_CLASS =
  'relative max-w-5xl mx-auto px-4 pt-6 pb-6 md:pt-4 md:pb-8' as const

export const LAYOUT_HERO_GLOW_CLASS =
  'pointer-events-none absolute inset-x-0 top-6 md:top-2 mx-auto h-[520px] max-w-3xl rounded-[40px] theme-hero-glow blur-3xl' as const
