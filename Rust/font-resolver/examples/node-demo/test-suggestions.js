const { getEnhancedSuggestions } = require('../../bindings/node/index');

async function runDemo() {
    console.log("=== Font Resolver Pro: Intelligent Suggestion Demo ===");
    console.log("Scenario: User is looking for 'Arial' (A very common font)");

    // 1. Search for a very common font (should hit thresholding)
    console.log("\nSearching for 'Arial'...");
    const arialResults = await getEnhancedSuggestions("Arial", true);

    console.log(`Found ${arialResults.length} suggestions.`);
    arialResults.forEach((res, i) => {
        const flag = res.isCriticalLicenseWarning ? "🚫 [License Warning]" : "✅ [Safe]";
        console.log(`${i + 1}. ${res.family} (${res.source}) - Score: ${(res.score * 100).toFixed(1)}% ${flag}`);
        if (i === 0 && res.fontFace) {
            console.log("   -> CSS Preview Block generated for the dropdown.");
        }
    });

    console.log("\nScenario: User is looking for an obscure font that needs internet search");
    console.log("Searching for 'Roboto Slab'...");
    const robotoResults = await getEnhancedSuggestions("Roboto Slab", true);

    robotoResults.slice(0, 5).forEach((res, i) => {
        const flag = res.isCriticalLicenseWarning ? "🚫 [License Warning]" : "✅ [Safe]";
        console.log(`${i + 1}. ${res.family} (${res.source}) - Score: ${(res.score * 100).toFixed(1)}% ${flag}`);
    });

    console.log("\n=== Strategy Applied ===");
    console.log("1. Perfect matches now limit alternatives to avoid clutter.");
    console.log("2. Smart licensing only flags potentially restricted commercial fonts.");
    console.log("3. Results are aggregated from Local, Google Fonts, and Fontsource.");
}

runDemo().catch(console.error);
