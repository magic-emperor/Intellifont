const fs = require('fs');
const path = require('path');

console.log('✅ intelliFont Engine Setup');
console.log('---------------------------');

try {
    const binaryPath = path.join(__dirname, 'intellifont-engine.node');
    if (fs.existsSync(binaryPath)) {
        console.log(`[OK] Native binary found: ${binaryPath}`);
        const stats = fs.statSync(binaryPath);
        console.log(`[OK] Size: ${(stats.size / 1024 / 1024).toFixed(2)} MB`);

        // Check for main entry point
        const mainPath = path.join(__dirname, 'index.js');
        if (fs.existsSync(mainPath)) {
            console.log(`[OK] JS Entry point found: ${mainPath}`);

            console.log('\n🚀 Installation Successful!');
            console.log('\nUsage Examples:');
            console.log('  const { identify_visual_font } = require("intellifont-engine");');
            console.log('  // See README.md for full API documentation');
        } else {
            console.warn('⚠️  Warning: index.js not found');
        }

    } else {
        console.error('❌ Error: Native binary not found. Build may have failed.');
        process.exit(1);
    }
} catch (e) {
    console.error('❌ Setup failed:', e);
    process.exit(1);
}
