# Intellifont — AI-Powered Font Recognition Engine

**Identify any font in seconds.** Intellifont uses AI-driven visual matching to recognize fonts from images, documents, websites, and web pages. Completely offline, private, and runs in your browser or Node.js. No internet required, no data sent to servers.

Works on screenshots, PDFs, Word documents, PowerPoint presentations, and images. Returns the top matching fonts with confidence scores.

---

## Where to Get Intellifont

**Choose your platform:**

| Platform | Installation | Use Case |
|---|---|---|
| **Chrome Extension** | [Download v2.0.0](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-chrome-2.0.0.zip) \| [Load unpacked](#browser-extension-setup) | Right-click on any image in Chrome |
| **Firefox Extension** | [Download v2.0.0](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-firefox-2.0.0.zip) \| [Load unpacked](#browser-extension-setup) | Right-click on any image in Firefox |
| **NPM Package** | `npm install intellifont-engine` | Identify fonts in Node.js apps |
| **WASM** | `npm install intellifont-wasm` | Identify fonts in browsers / serverless |
| **Server API** | `git clone && npm start` | REST API on localhost:3001 |

---

## What It Can Do

| Input | Method | Accuracy |
|---|---|---|
| **Font file** (TTF/OTF/WOFF) | Direct file analysis | ~95% |
| **Website font** | URL inspection | ~95% |
| **PDF / Document** | Embedded font extraction | ~95% |
| **Text in image** | Visual analysis | 92% top-1 \* |

\* *Accuracy varies by image quality. We actively improve this with every release.*

---

## Quick Start

### For Users — Browser Extension
**Chrome & Firefox** — load unpacked (coming to stores):

1. Clone: `git clone https://github.com/magic-emperor/Intellifont.git`
2. **Chrome**: `chrome://extensions`  Developer Mode  Load unpacked  select `extension/`
3. **Firefox**: `about:debugging`  Load Temporary Add-on  select `extension-firefox/manifest.json`
4. Right-click any image  **"Identify font in image"**

### For Developers — NPM Package

```bash
npm install intellifont-engine
```

```javascript
const intellifont = require('intellifont-engine');

// Identify font from a file
const matches = intellifont.identifyVisualFont('./sample.ttf', 'Hello');
console.log(matches);
// Output:
// [
//   { family: 'Roboto', confidence: 0.98 },
//   { family: 'Open Sans', confidence: 0.94 },
//   ...
// ]
```

### WASM (Browser / Serverless)

```bash
npm install intellifont-wasm
```

```javascript
import init from 'intellifont-wasm';

const engine = await init();
const results = engine.identifyFont(glyphMetrics, 8);
```

---

## Browser Extension Setup

### Installing the Chrome Extension

1. **Download** the extension: [intellifont-chrome-2.0.0.zip](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-chrome-2.0.0.zip)
2. **Extract** the ZIP file to a folder (e.g., `Downloads/intellifont-chrome`)
3. Open **Chrome** and go to `chrome://extensions`
4. Enable **Developer Mode** (toggle in top-right corner)
5. Click **Load unpacked**
6. Select the extracted `intellifont-chrome` folder
7. Extension appears in your toolbar
8. **Use it**: Right-click any image  "Identify font in image"

### Installing the Firefox Extension

1. **Download** the extension: [intellifont-firefox-2.0.0.zip](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-firefox-2.0.0.zip)
2. **Extract** the ZIP file to a folder (e.g., `Downloads/intellifont-firefox`)
3. Open **Firefox** and go to `about:debugging`
4. Click **This Firefox** (left sidebar)
5. Click **Load Temporary Add-on**
6. Select any file from the extracted `intellifont-firefox` folder (e.g., `manifest.json`)
7. Extension appears in your toolbar
8. **Use it**: Right-click any image  "Identify font"

### How to Use the Extension

1. **Right-click on any image** on any webpage
2. Select **"Identify font in image"** (or "Identify font in image region..." for partial image)
3. Wait for analysis (usually 1-2 seconds)
4. A popup appears with:
   - **Top matching fonts** (confidence % shown)
   - **Font family, weight, style** for each match
   - Copy button for easy access

### Known Extension Limitations

- Only works on images you can right-click (no cross-origin restrictions thanks to local processing)
- Image must contain **readable text** (8px+ font size)
- Accuracy varies by image quality (see [Accuracy](#accuracy-by-condition))
- **Rotation**: Deskew images first if heavily tilted

---

## Delivery Channels

###  **Browser Extension** (Chrome & Firefox)
Right-click on images  identify fonts. Offline first, 100% private.

**Download:**
- **Chrome**: [Download v2.0.0 ZIP](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-chrome-2.0.0.zip)
- **Firefox**: [Download v2.0.0 ZIP](https://github.com/magic-emperor/Intellifont/releases/download/v2.0.0/intellifont-firefox-2.0.0.zip)

**Installation** (see [Browser Extension Setup](#browser-extension-setup) below):
1. Extract the ZIP file
2. Load unpacked in Chrome/Firefox Developer Mode
3. Right-click any image  "Identify font"

**Coming soon**: Chrome Web Store & Firefox Add-ons (submit after testing)

###  **NPM Package** (`intellifont-engine`)
```bash
npm install intellifont-engine
```

20+ functions for font identification, analysis, and caching:
- `identifyVisualFont()` — Identify from TTF/OTF file
- `identifyVisualFontBuffer()` — Identify from buffer
- `aiSuggestSimilar()` — Find similar fonts (ML)
- `getFontSuggestions()` — Name-based search
- `analyzeImage()` — Full image processing

**TypeScript types included.**

###  **WASM Module**
Pure JavaScript, zero native dependencies. Perfect for browsers, serverless, Deno.

```bash
npm install intellifont-wasm
```

###  **API Server**
```bash
cd server && npm start
# POST /api/identify — upload image
# GET /api/identify-url?url=... — scan web page
# POST /api/identify-document — analyze PDF/DOCX
# GET /api/similar?font=Roboto — find alternatives
```

---

## Accuracy by Condition

| Scenario | Accuracy |
|---|---|
| Font file (TTF/OTF) | ~95% |
| Web font (downloaded) | ~95% |
| PDF/Document embedded font | ~95% |
| Image: clean text | 92% top-1 |
| Image: mixed conditions | 57% top-5 |

*Image accuracy depends on text quality, size, contrast, and clarity. Rotation is a known limitation (improving).*

---

## How It Works

```
Input (Image / Document / Font / URL)
       
[Preprocessing]   Binarize, deskew, extract text regions
       
[Glyph Analysis]  Compute pixel metrics (density, symmetry, strokes, serif)
       
[Visual Matching]  Compare 64—64 thumbnails against 2,042 fonts
       
[ML Enhancement]  Optional: CosFace embeddings for better ranking
       
[Return Top 8]    Sorted by confidence score
       
Output: Font family, weight, confidence %
```

**Key points:**
- **Visual matching**: Character-agnostic (label-independent)
- **ML-powered**: Trained on 2,042 real fonts
- **Offline**: All data bundled, no network calls
- **Private**: Images never leave your device

---

## Full API Reference

### Node.js Exports

**Core Identification**
```javascript
identifyVisualFont(fontPath: string, characters: string, limit?: number): VisualMatch[]
identifyVisualFontBuffer(buffer: Buffer, characters: string, limit?: number): VisualMatch[]
aiSuggestSimilar(fontPath: string, limit?: number): AiSuggestion[]
getFontSuggestions(fontName: string, includeInternet?: boolean): Promise<Suggestion[]>
```

**Image Processing**
```javascript
analyzeImage(imageSource: string | Buffer): Promise<ImageAnalysisResult>
extractGlyphSignature(fontPath: string, character: string): GlyphSignature
compareGlyphSignatures(fontPathA: string, fontPathB: string, character: string): number
```

**Utilities**
```javascript
normalizeFontName(fontName: string): string
```

**Cache Management**
```javascript
pinFont(fontName: string): void
unpinFont(fontName: string): void
listPinnedFonts(): string[]
cleanupCache(aggressive?: boolean): number
getCacheStats(): CacheStats
getEngineStats(): EngineStats
```

Full TypeScript types in `index.d.ts`.

---

## Browser Support

| Browser | Version | Status |
|---|---|---|
| Chrome | 80+ |  Full |
| Firefox | 109+ |  Full |
| Edge | 80+ |  Full |
| Safari | 14+ |  Planned |

WASM module works on 99%+ of users' devices.

---

## Database

**2,042 fonts** covering:
- Google Fonts (complete)
- System fonts (Windows, macOS, Linux)
- Popular design fonts
- Open-source typefaces

**Included in extension and NPM:**
- Glyph signatures: 2 MB
- Pixel signatures: 1.7 MB
- Visual thumbnails: 15 MB

---

## Performance

| Task | Time | Notes |
|---|---|---|
| Identify 1 glyph | 5-50ms | CPU-bound |
| Full image analysis | 100-500ms | Includes preprocessing |
| ML similarity ranking | 10-100ms | Optional |

No initialization delay. Ready immediately.

---

## Privacy & Security

 **100% Offline** — All processing on your device  
 **No Tracking** — Zero telemetry  
 **No Uploads** — Images never leave your computer  
 **Open Source** — Inspect the code  
 **Apache 2.0** — Free for commercial use  

---

## Installation

### From NPM
```bash
npm install intellifont-engine
```

### From Source (Development)
```bash
git clone https://github.com/magic-emperor/Intellifont.git
cd Intellifont

# Build Rust bindings
cd Rust/font-resolver
cargo build --release
cd bindings/node
npm install
npm run build
```

---

## Known Limitations

- **Rotation**: Tilted text is difficult (deskew first)
- **Rare fonts**: Only covers 2,042 fonts (expand DB for your use case)
- **Stylized text**: Heavily styled/outlined text has lower accuracy
- **Small text**: < 8px is hard to analyze

---

## Contributing

Found a bug? Want to help? Open an issue or PR on [GitHub](https://github.com/magic-emperor/Intellifont).

---

## License

**Apache License 2.0.** See [LICENSE](LICENSE).

---

## Credits

- **Rust** engine with NAPI-RS bindings
- **WebAssembly** for browser support
- **CosFace** ML embeddings
- **2,042 test fonts** for training

---

** 2026 Intellifont Engine. Built by the Intellifont Team.**

Questions? Email [faizan77603@gmail.com](mailto:faizan77603@gmail.com) or open an issue on [GitHub](https://github.com/magic-emperor/Intellifont).

