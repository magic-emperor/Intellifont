# intelliFont Engine

**The Ultimate Professional Font Identification Toolkit.**

`intellifont-engine` is a high-performance, Rust-powered engine designed to **visually identify** unknown font files. Whether you are building a design tool or a font management system, this engine tells you exactly what font family a file belongs to by analyzing its glyph shapes.

---

## Why intelliFont?
**intelliFont** uses **Visual DNA Analysis**—measuring the physical curves, stroke width, and ink density of the characters—to fingerprint and identify the font family with high precision.

Traditional tools rely on reading metadata (names), which can be missing, scrambled, or incorrect (e.g., "subset.ttf").



### Key Capabilities:
- **Visual Recognition**: Identifies fonts based on *geometry*, not file names.
- **AI Similarity Suggestion**: **[NEW]** Find visually similar alternatives matching the "AI DNA" of a font.
- **Microservice Ready**: **[NEW]** Run as a high-performance HTTP server for any-language integration.
- **Raw Buffer Support**: Works directly with memory buffers from web uploads.
- **Sub-millisecond Speed**: Visual analysis takes <1ms per file.
- **Privacy First**: Analysis happens entirely offline/on-server.

---

## Architecture Overview

The engine is built on a "Service-Core" architecture:
1.  **The Rust Core**: Performs the heavy-duty mathematical analysis of glyph curves.
2.  **The NAPI-RS Bridge**: Exposes this power to Node.js as a simple native function.
3.  **The Signature Database**: A highly-compressed binary index of known font shapes (Google Fonts, System Fonts).

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

## JavaScript API

`intellifont-engine` provides a simple, Promise-based API for Node.js environments.

### `identify_visual_font(path, chars, limit)`
**[NEW]** Identify a font file visually using its glyph shape signatures.

```javascript
const { identify_visual_font } = require('intellifont-engine');

// Identify an unknown file
const results = identify_visual_font("./unknown_font.ttf", "RQWM", 5);

console.log(results[0]);
// {
//   family: "Arial",
//   subfamily: "Regular",
//   confidence: 0.99,
//   source: "Database"
// }
```

### `identify_visual_font_buffer(buffer, chars, limit)`
**[NEW]** Identify a font from valid font memory buffer (e.g. from file upload). No disk save required.

```javascript
const fs = require('fs');
const { identify_visual_font_buffer } = require('intellifont-engine');

// Simulate a file upload (Buffer)
const buffer = fs.readFileSync("./uploaded_font.ttf");

const matches = identify_visual_font_buffer(buffer, "RQWM", 5);
console.log(matches[0]);
// { family: "Roboto", confidence: 1.0, ... }
```

### `aiSuggestSimilar(path, limit)`
**[NEW]** Find visually similar fonts using AI pattern matching.

```javascript
const { aiSuggestSimilar } = require('intellifont-engine');

const suggestions = aiSuggestSimilar("./my_font.ttf", 5);
console.log(suggestions[0]);
// {
//   family: "Consolas",
//   confidence: 0.92,
//   match_quality: "high"
// }
```

### `aiSuggestSimilarBuffer(buffer, limit)`
**[NEW]** Find similar fonts directly from memory buffer.
```

### `exportMetrics(fontName)`
Returns the physical "DNA" of a font. Essential for layout-stable PDF generators.

```javascript
const { exportMetrics } = require('intellifont-engine');
const metrics = exportMetrics("Inter");
```

---

## 📦 Integration: Real-World Web App

Here is a complete example of how to integrate `intellifont-engine` into an **Express.js** backend to handle file uploads.

### 1. Install Dependencies
```bash
npm install express multer intellifont-engine
```

### 2. Create the Backend Service (`server.js`)
This service receives a file upload (font) and returns the identification result.

```javascript
const express = require('express');
const multer = require('multer');
const { identify_visual_font_buffer } = require('intellifont-engine');

const app = express();
// Use memory storage so we can access the buffer directly
const upload = multer({ storage: multer.memoryStorage() });

// Endpoint: POST /api/identify
app.post('/api/identify', upload.single('fontFile'), (req, res) => {
    try {
        if (!req.file) {
            return res.status(400).json({ error: "No font file uploaded" });
        }

        // 1. Get the Raw Buffer
        const fontBuffer = req.file.buffer;

        // 2. Identify the Font (Pass specific characters for better accuracy)
        const results = identify_visual_font_buffer(fontBuffer, "RQWM", 5);

        // 3. Return JSON to Frontend
        res.json({ success: true, matches: results });

    } catch (error) {
        res.status(500).json({ error: error.message });
    }
});

app.listen(3000, () => console.log('Server running on port 3000 🚀'));
```

### 3. Usage from Frontend
```javascript
const formData = new FormData();
formData.append('fontFile', fileInput.files[0]);

const response = await fetch('/api/identify', { method: 'POST', body: formData });
const data = await response.json();
console.log("Identified Font:", data.matches[0].family); 
// Output: "Arial"
```

---

## ❓ FAQ: Inputs & Performance

**Q: Why do I need to pass the `.ttf/.otf` file? Can't I just pass text?**
**A:** "Text" (like "Hello") is just a code (e.g., `U+0048`). It has no shape. The `.ttf` file contains the **mathematical curves** (geometry) that determine if it looks like Arial or Times New Roman. We need those curves to calculate the visual signature.

**Q: Isn't uploading a font file slow/heavy?**
**A:** No. Font files are vector graphics and are surprisingly small (~50KB - 200KB). Uploading a font buffer to your backend takes milliseconds.

---

## 🛠️ CLI Usage (The 'intellifont' command)

Once installed globally, `intellifont` provides tools for visual identification and database management.

| Command | Description |
| :--- | :--- |
| `intellifont identify "file.ttf"` | **Identify a font file visually** using its glyph signatures. |
| `intellifont ai-suggest "file.ttf"` | **[AI]** Find visually similar alternatives for a font file. |
| `intellifont serve --port 3000` | **[NEW]** Run as an HTTP microservice for universal integration. |
| `intellifont build-web-db` | Download and index popular **Google Fonts/Web Fonts** automatically. |
| `intellifont build-glyph-db <dir>` | Index a local directory of fonts into a compressed signature database. |
| `intellifont stats` | Display engine metrics and database compression stats. |

### JSON Output for Automation
Add `--json` flag to get machine-readable output:
```bash
intellifont identify "file.ttf" --json
# Returns: [{"family": "Arial", "confidence": 0.99, ...}]
```

---

## 🐍 Python & Universal Integration

Since `intellifont` is a standalone CLI tool, you can use it from **any language** (Python, Go, PHP, C#, Ruby) by calling the CLI and parsing JSON.

**Python Example:**
```python
import subprocess
import json

def identify_font(file_path):
    result = subprocess.run(
        ["intellifont", "identify", file_path, "--json"],
        capture_output=True, text=True
    )
    matches = json.loads(result.stdout)
    return matches[0] if matches else None

# Usage
font = identify_font("./mystery.ttf")
print(f"Identified: {font['family']} ({font['confidence']*100:.0f}% confidence)")
```


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
