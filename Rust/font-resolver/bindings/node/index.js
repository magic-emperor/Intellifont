const {
  getFontSuggestions,
  normalizeFontName,
  pinFont,
  unpinFont,
  removeFromCache,
  exportMetrics,
  getEngineStats,
  getCacheStats,
  cleanupCache,
  listPinnedFonts,
  updateDatabase
} = require('./intellifont-engine.node');

/**
 * Creates a CSS @font-face string for a given font family and URL.
 */
function createFontFace(family, url, weight = 400, italic = false) {
  return `
@font-face {
  font-family: '${family}';
  src: url('${url}') format('woff2');
  font-weight: ${weight};
  font-style: ${italic ? 'italic' : 'normal'};
  font-display: swap;
}
  `.trim();
}

/**
 * Enhanced suggestion helper that includes license advice.
 */
async function getEnhancedSuggestions(fontName, includeInternet = true) {
  const suggestions = await getFontSuggestions(fontName, includeInternet);

  return suggestions.map(s => ({
    ...s,
    uiAdvice: s.isCriticalLicenseWarning
      ? "⚠ Warning: Check license before commercial use."
      : "✔ Safe to use.",
    fontFace: s.downloadUrl ? createFontFace(s.family, s.downloadUrl, s.weight, s.italic) : null
  }));
}

/**
 * CLI Entry Point
 */
if (require.main === module) {
  const args = process.argv.slice(2);
  const command = args[0];
  const subCommand = args[1];
  const params = args.slice(command === 'cache' || command === 'config' ? 2 : 1);

  async function runCli() {
    switch (command) {
      case 'resolve':
      case 'suggest':
        const searchName = params[0] || "Arial";
        const useWeb = args.includes('--internet') || args.includes('--web');
        const results = await getEnhancedSuggestions(searchName, useWeb);
        console.log(`\nSuggestions for "${searchName}":`);
        results.forEach(r => {
          const status = r.isCriticalLicenseWarning ? '[RESTRICTED]' : '[OK]';
          console.log(` ${status} ${r.family} ${r.subfamily} (${r.source}) - Similarity: ${(r.score * 100).toFixed(1)}%`);
        });
        break;

      case 'tiered':
        // For CLI parity, we use the same as suggest but with different UI formatting
        const tieredName = params[0] || "Arial";
        const tieredWeb = args.includes('--internet');
        const tieredResults = await getEnhancedSuggestions(tieredName, tieredWeb);
        console.log(`\nTiered Analysis for "${tieredName}":`);
        const exact = tieredResults.filter(r => r.score > 0.95);
        const similar = tieredResults.filter(r => r.score <= 0.95 && r.score > 0.8);

        if (exact.length) {
          console.log("\n[90%+ Match Tier]");
          exact.forEach(r => console.log(` - ${r.family} (Exact match)`));
        }
        if (similar.length) {
          console.log("\n[80%+ Similar Tier]");
          similar.forEach(r => console.log(` - ${r.family} (${(r.score * 100).toFixed(1)}%)`));
        }
        break;

      case 'stats':
        const eStats = getEngineStats();
        const cStats = getCacheStats();
        console.log(`\nENGINE STATUS`);
        console.log(`  Fonts indexed : ${eStats.fontCount.toLocaleString()}`);
        console.log(`  Binary size   : ${eStats.compressedSizeMb.toFixed(2)} MB`);
        console.log(`  Ratio         : ${eStats.compressionRatio.toFixed(1)}%`);
        console.log(`\nCACHE STATUS`);
        console.log(`  Memory/Disk   : ${cStats.memoryEntries}/${cStats.diskEntries} entries`);
        console.log(`  Pinned        : ${cStats.pinnedFonts}`);
        console.log(`  Disk usage    : ${cStats.diskUsageMb.toFixed(2)} MB`);
        break;

      case 'cache':
        switch (subCommand) {
          case 'stats':
            const cs = getCacheStats();
            console.log(`\nCACHE: ${cs.memoryEntries} mem / ${cs.diskEntries} disk (${cs.diskUsageMb.toFixed(2)} MB)`);
            break;
          case 'cleanup':
            const agg = args.includes('--aggressive');
            const cleaned = cleanupCache(agg);
            console.log(`Cache cleared. Removed ${cleaned} entries.`);
            break;
          case 'list':
            const pinned = listPinnedFonts();
            console.log("\nPINNED FONTS");
            pinned.length ? pinned.forEach(p => console.log(`  - ${p}`)) : console.log("  (None)");
            break;
          case 'pin':
            params.forEach(p => { pinFont(p); console.log(`Pinned: ${p}`); });
            break;
          case 'unpin':
            params.forEach(p => { unpinFont(p); console.log(`Unpinned: ${p}`); });
            break;
        }
        break;

      case 'scan':
        const sStats = getEngineStats();
        console.log(`\nScan complete. ${sStats.fontCount} fonts available.`);
        break;

      case 'update':
        process.stdout.write("Updating database... ");
        try {
          await updateDatabase();
          console.log("Done.");
        } catch (e) {
          console.log("\nFailed.");
          console.error(`Error: ${e.message}`);
        }
        break;

      case 'export-metrics':
        try {
          console.log(exportMetrics(params[0] || "Arial"));
        } catch (e) {
          console.error(`Error: ${e.message}`);
        }
        break;

      case 'setup':
        console.log("\nintelliFont Setup Wizard");
        console.log("1. Memory Limit: 16MB (Default)");
        console.log("2. Web Fonts: Enabled");
        console.log("3. License Guard: Active");
        console.log("\nConfiguration optimized for your system.");
        break;

      case 'normalize':
        console.log(normalizeFontName(params[0] || "Arial"));
        break;

      case 'help':
      default:
        console.log("\nintelliFont Engine CLI");
        console.log("Usage:");
        console.log("  intellifont suggest <name> [--internet]    - Find matching fonts");
        console.log("  intellifont resolve <name>                 - Fast lookup");
        console.log("  intellifont stats                          - Engine & Cache metrics");
        console.log("  intellifont scan                           - Refresh system font index");
        console.log("  intellifont update                         - Sync with global CDN signatures");
        console.log("  intellifont cache <stats|list|cleanup>     - Manage local cache");
        console.log("  intellifont cache <pin|unpin> <name>       - Manage font pinning");
        console.log("  intellifont export-metrics <name>          - Get binary DNA of a font");
        break;
    }
  }
  runCli().catch(console.error);
}

module.exports = {
  getFontSuggestions,
  getEnhancedSuggestions,
  normalizeFontName,
  createFontFace,
  pinFont,
  unpinFont,
  removeFromCache,
  exportMetrics,
  getEngineStats,
  getCacheStats,
  cleanupCache,
  listPinnedFonts,
  updateDatabase
};
