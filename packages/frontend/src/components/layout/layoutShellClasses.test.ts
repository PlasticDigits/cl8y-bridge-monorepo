import { describe, expect, it } from 'vitest'
import { LAYOUT_HERO_GLOW_CLASS, LAYOUT_MAIN_CLASS } from './layoutShellClasses'

describe('layoutShellClasses (INV-MOB1, GL-126)', () => {
  it('uses extra main top padding below md; restores md rhythm', () => {
    expect(LAYOUT_MAIN_CLASS).toContain('pt-6')
    expect(LAYOUT_MAIN_CLASS).toContain('md:pt-4')
    expect(LAYOUT_MAIN_CLASS).not.toContain('pt-3')
  })

  it('anchors hero glow lower on narrow viewports to reduce blur bleed toward the nav', () => {
    expect(LAYOUT_HERO_GLOW_CLASS).toContain('top-6')
    expect(LAYOUT_HERO_GLOW_CLASS).toContain('md:top-2')
  })
})
