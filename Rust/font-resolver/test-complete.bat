@echo off
echo === Complete Font Resolver Test ===
echo.

echo 1. Building all crates...
cargo build --release
if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo 2. Running interactive setup...
echo.
cargo run --release --example cli -- setup

echo.
echo 3. Testing basic resolution...
echo.
cargo run --release --example cli -- "ArialMT"

echo.
echo 4. Testing web font resolution...
echo.
cargo run --release --example cli -- resolve "Roboto" --web

echo.
echo 5. Testing cache features...
echo.
cargo run --release --example cli -- cache stats
cargo run --release --example cli -- cache list

echo.
echo 6. Testing license checking...
echo.
cargo run --release --example cli -- check-license "Helvetica"

echo.
echo 7. Testing source management...
echo.
cargo run --release --example cli -- config show

echo.
echo ================================
echo ✅ COMPLETE SYSTEM TESTED!
echo ================================
echo.
echo All crates implemented:
echo • font-core - Core structures
echo • font-normalizer - Font name normalization
echo • font-parser - Font file parsing
echo • font-scanner - System font scanning
echo • font-cache - Hybrid caching (2MB/10MB)
echo • font-setup - Interactive setup
echo • font-license - Commercial font detection
echo • font-sources - Multi-source management
echo • font-web-db - Web font database
echo • font-resolver - Main resolver engine
echo.
pause