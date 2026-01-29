# intelliFont Engine

**The Ultimate Professional Font Recognition & Suggestion Toolkit.**

`intellifont-engine` is a high-performance, Rust-powered engine designed for mission-critical design applications. Whether you are building a PDF editor, a web builder, or a design tool, this engine ensures that no font ever goes missing and every layout remains pixel-perfect.

---

## Why intelliFont?

Traditional font matching relies on exact string comparisons, which fail if the user's system names a font slightly differently (e.g., "HelveticaNeue" vs "Helvetica Neue"). 

**intelliFont** uses **Biometric Metric Matching**—analyzing the physical architecture of the font itself (ascenders, descenders, x-height, and cap-height)—to find the perfect match or the best possible substitute.

### Key Capabilities:
- **Blazing Speed**: Rust-core execution means sub-1ms response times for local lookups.
- **Pre-Seeded Database**: Comes with **2,000+ popular font signatures** (Google Fonts, System Standards) embedded in a highly compressed 4.5MB binary asset.
- **Layout Authority**: Exports precise physical metrics to ensure PDF and Canvas layouts never shift during font substitution.
- **Hybrid Intelligence**: Scans local OS fonts and queries global CDNs (Google, Fontsource) in parallel.
- **Legal Guardrails**: Real-time license detection for OFL, Apache, and commercial fonts, protecting users from legal risks.

---

## Architecture Overview

The engine is built on a "Service-Core" architecture:
1.  **The Rust Core**: Handles heavy-duty binary parsing, Brotli decompression of the signature database, and parallel network queries using Tokio.
2.  **The NAPI-RS Bridge**: Provides a zero-copy memory bridge between Rust and Node.js, ensuring that huge font lists don't cause garbage collection lag.
3.  **The Metadata Cache**: A dual-layer cache (Memory + Persistent Disk) that learns from every search, making web results instant and offline-ready on the next launch.

---

##  Installation

### As a Library (Recommended)
Add it to your React, Vue, or Node.js project:
```bash
npm install intellifont-engine
```

### As a Global CLI Tool
Powerful font management directly from your terminal:
```bash
npm install -g intellifont-engine
```

---

## 🛠️ CLI Usage (The 'intellifont' command)

Once installed globally, `intellifont` provides a robust set of tools for developers and system administrators.

### Core Commands

| Command | Description |
| :--- | :--- |
| `intellifont stats` | Display exhaustive engine metrics: font counts, database compression ratio, and cache health. |
| `intellifont resolve "Font"` | Performs a high-speed resolution using local assets and the metadata cache. |
| `intellifont tiered "Font"` | Advanced similarity analysis. Returns results in 90% (Exact) and 80% (Similar) tiers. |
| `intellifont tiered "Font" --internet` | Activate global CDN lookup (Google Fonts, Fontsource) if local matches are insufficient. |
| `intellifont find-similar "Font"` | Identify fonts that are visually or metrically similar to a target baseline font. |
| `intellifont check-license "Font"` | Analyze licensing metadata for commercial safety and provide free equivalents. |
| `intellifont scan [--detailed]` | Deep recursive scan of system font directories to index new assets. |
| `intellifont update` | Synchronize local signatures with the global repository and regenerate binary indexes. |
| `intellifont setup` | Guided 3-step configuration for memory limits and provider priorities. |
| `intellifont version` | Display engine version, build architecture, and capability flags. |

### Cache Management (`intellifont cache <command>`)

| Subcommand | Description |
| :--- | :--- |
| `cache stats` | View hit rates, memory footprint, and disk usage of the local cache. |
| `cache cleanup` | Remove stale entries. Use `--aggressive` to clear single-use fonts. |
| `cache pin "Font"` | Lock a font permanently in the cache so it is never evicted. |
| `cache unpin "Font"` | Release a font from the permanent cache lock. |
| `cache list` | List all manually and automatically pinned (highly used) fonts. |
| `cache suggest` | Analyze usage patterns and suggest non-pinned entries for manual removal. |

### Configuration (`intellifont config <command>`)

| Subcommand | Description |
| :--- | :--- |
| `config show` | View current engine limits (Memory, Disk, Web Access). |
| `config set <key> <val>` | Directly modify configuration (e.g., `intellifont config set memory_limit 16`). |
| `config reset` | Restore all engine settings to factory defaults. |
| `config export <path>` | Export your current setup to a `.toml` file for team sharing. |
| `config import <path>` | Load a shared configuration file to synchronize settings across environments. |

---

## JavaScript API

`intellifont-engine` provides a simple, Promise-based API for Node.js environments.

### `getEnhancedSuggestions(name, useWebFonts)`
The primary entry point for building "Suggested Fonts" dropdowns.

```javascript
const { getEnhancedSuggestions } = require('intellifont-engine');

async function resolveFont(inputName) {
  // Parallel search: Local System + Global Fontsource/Google
  const results = await getEnhancedSuggestions(inputName, true);
  
  results.forEach(res => {
    console.log(`Match Found: ${res.family}`);
    console.log(`Similarity: ${(res.score * 100).toFixed(1)}%`);
    
    // License Guardrail
    if (res.isCriticalLicenseWarning) {
      console.warn("🚫 Commercial License Required - Consider a free alternative.");
    }
  });
}
```

### `exportMetrics(fontName)`
Returns the physical "DNA" of a font. Essential for layout-stable PDF generators.

```javascript
const { exportMetrics } = require('intellifont-engine');

const metrics = exportMetrics("Inter");
// Returns:
// {
//   family: "Inter",
//   ascender: 0.72,
//   descender: -0.21,
//   capHeight: 0.68,
//   widthFactor: 1.05
// }
```

---

## Advanced Features

### Triple-Layer Safety Net
If a font is missing, intelliFont executes its survival strategy:
1.  **Direct Local Match**: Scans the user's OS fonts instantly.
2.  **CDN Deep Search**: Queries Google Fonts and Fontsource in parallel (2-second timeout).
3.  **Metric Substitution**: If nowhere to be found, it analyzes the missing font's metrics and finds a "Twin" on the local system to prevent layout breaking.

### Learning Mode (Dynamic Caching)
The engine learns your project's typography. Any font resolved from the web is automatically "fingerprinted" and stored in the **Persistent Disk Cache**. Next time you search for that font, it will be found instantly—even without an internet connection.

---

## 💰 Licensing & Support

`intellifont-engine` is dual-licensed:
- **Software**: The engine code is licensed under **Apache-2.0**.
- **Data**: The included **Core Font Database** (~2,000 signatures) is free for use. Access to the **Extended Intelligence Database** (20,000+ signatures) requires a Pro license.

For **Enterprise-grade** requirements:
- 🚀 **20,000+ Certified Font Signatures**
- 🔒 **Air-gapped Offline Databases** (for secure environments)
- 🎧 **Premium Support & Custom Provider Integration**

Visit [intellifont.pro](https://intellifont.pro) or contact our enterprise team.

---

## Security

Your privacy is paramount. intelliFont's local scanner:
- Is restricted to common system font directories.
- Never transmits actual font files; only anonymous biometric signatures (metrics).
- Uses Rust's memory safety to prevent overflow exploits common in font parsing.

---

© 2026 intelliFont Engine
