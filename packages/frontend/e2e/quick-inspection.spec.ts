/**
 * Quick inspection test - just checks the page without full E2E setup
 * Run with: npx playwright test quick-inspection.spec.ts --project=chromium
 */
import { test, expect } from '@playwright/test'

test.describe('Bridge DApp Quick Inspection', () => {
  test('should load page and show chain information', async ({ page }) => {
    console.log('\n=== Starting page inspection ===\n')
    
    // Navigate to the page
    await page.goto('http://localhost:3000/')
    console.log('✓ Navigated to http://localhost:3000/')

    // Wait for the page to load
    await page.waitForLoadState('networkidle')
    console.log('✓ Page loaded')

    // Take initial screenshot
    await page.screenshot({ path: 'test-results/quick-01-initial-load.png', fullPage: true })
    console.log('✓ Screenshot saved: test-results/quick-01-initial-load.png')

    // Get the full page content
    const pageContent = await page.content()
    const bodyText = await page.locator('body').textContent()
    
    console.log('\n=== Checking for chain names ===')
    
    // Check for mainnet chain names
    const chainNames = ['Terra Classic', 'BNB Chain', 'opBNB', 'Ethereum', 'Polygon', 'Avalanche']
    const foundChains: string[] = []
    
    for (const chainName of chainNames) {
      if (pageContent.includes(chainName) || bodyText?.includes(chainName)) {
        foundChains.push(chainName)
        console.log(`✓ Found: ${chainName}`)
      }
    }
    
    if (foundChains.length === 0) {
      console.log('✗ No mainnet chain names found')
    } else {
      console.log(`\nTotal chains found: ${foundChains.join(', ')}`)
    }
    
    console.log('\n=== Checking for MAX button ===')
    
    // Check for MAX button
    const maxButtons = await page.locator('button:has-text("MAX"), button:has-text("Max"), button:has-text("max")').all()
    console.log(`Found ${maxButtons.length} MAX button(s)`)
    
    if (maxButtons.length > 0) {
      for (let i = 0; i < maxButtons.length; i++) {
        const isVisible = await maxButtons[i].isVisible()
        console.log(`  MAX button ${i + 1}: ${isVisible ? 'visible' : 'hidden'}`)
      }
    }
    
    console.log('\n=== Checking for errors ===')
    
    // Check for error messages
    const errorSelectors = [
      '[class*="error"]',
      '[role="alert"]',
      '.error-message',
      '[data-testid*="error"]'
    ]
    
    let totalErrors = 0
    for (const selector of errorSelectors) {
      const errors = await page.locator(selector).all()
      if (errors.length > 0) {
        for (const error of errors) {
          const errorText = await error.textContent()
          if (errorText && errorText.trim()) {
            console.log(`  ✗ Error found: ${errorText.trim()}`)
            totalErrors++
          }
        }
      }
    }
    
    if (totalErrors === 0) {
      console.log('  ✓ No errors found')
    }
    
    console.log('\n=== Checking page structure ===')
    
    // Check for common UI elements
    const elements = {
      'FROM section': await page.locator('[class*="from"], [data-testid*="from"]').count(),
      'TO section': await page.locator('[class*="to"], [data-testid*="to"]').count(),
      'Amount input': await page.locator('input[type="text"], input[type="number"]').count(),
      'Buttons': await page.locator('button').count(),
    }
    
    for (const [name, count] of Object.entries(elements)) {
      console.log(`  ${name}: ${count} element(s)`)
    }
    
    // Take final screenshot
    await page.screenshot({ path: 'test-results/quick-02-after-inspection.png', fullPage: true })
    console.log('\n✓ Final screenshot saved: test-results/quick-02-after-inspection.png')
    
    console.log('\n=== Page title ===')
    const title = await page.title()
    console.log(`  "${title}"`)
    
    console.log('\n=== Inspection complete ===\n')
  })
})
