/**
 * Manual inspection test - verifies page loads and shows expected chain names
 */
import { test, expect } from '@playwright/test'

test.describe('Bridge DApp Manual Inspection', () => {
  test('should load page and show mainnet chain names', async ({ page }) => {
    // Navigate to the page
    await page.goto('/')

    // Wait for the page to load
    await page.waitForLoadState('networkidle')

    // Take a screenshot
    await page.screenshot({ path: 'test-results/01-initial-load.png', fullPage: true })

    // Check for FROM chain selector
    const fromChainSelector = page.locator('[data-testid="from-chain-selector"], .from-chain, [class*="FromChain"]').first()
    
    // Check for TO chain selector
    const toChainSelector = page.locator('[data-testid="to-chain-selector"], .to-chain, [class*="ToChain"]').first()

    // Check for MAX button near amount input
    const maxButton = page.locator('button:has-text("MAX"), button:has-text("Max")').first()

    // Get text content from chain selectors
    const pageContent = await page.content()
    
    console.log('\n=== PAGE INSPECTION RESULTS ===\n')
    
    // Check if Terra Classic appears
    const hasTerraClassic = pageContent.includes('Terra Classic')
    console.log(`Terra Classic found: ${hasTerraClassic}`)
    
    // Check if BNB Chain appears
    const hasBNBChain = pageContent.includes('BNB Chain')
    console.log(`BNB Chain found: ${hasBNBChain}`)
    
    // Check if opBNB appears
    const hasOpBNB = pageContent.includes('opBNB')
    console.log(`opBNB found: ${hasOpBNB}`)
    
    // Check MAX button visibility
    const maxButtonVisible = await maxButton.isVisible().catch(() => false)
    console.log(`MAX button visible: ${maxButtonVisible}`)
    
    // Check for any error messages
    const errorElements = await page.locator('[class*="error"], [role="alert"], .error-message').all()
    console.log(`Error elements found: ${errorElements.length}`)
    
    if (errorElements.length > 0) {
      for (const error of errorElements) {
        const errorText = await error.textContent()
        console.log(`  - Error: ${errorText}`)
      }
    }
    
    // Get all text from chain selector areas
    const allText = await page.locator('body').textContent()
    const chainMatches = allText?.match(/(Terra Classic|BNB Chain|opBNB|Ethereum|Polygon|Avalanche)/g) || []
    console.log(`\nChain names found on page: ${[...new Set(chainMatches)].join(', ')}`)
    
    // Take final screenshot
    await page.screenshot({ path: 'test-results/02-after-inspection.png', fullPage: true })
    
    console.log('\n=== END INSPECTION ===\n')
    
    // Basic assertions
    expect(hasTerraClassic || hasBNBChain || hasOpBNB).toBeTruthy()
  })
})
