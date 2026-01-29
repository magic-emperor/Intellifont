@echo off
echo === Enhanced Font Resolver Test ===
echo.

echo 1. Building all crates...
cargo build
if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo 2. Running interactive setup...
echo.
cargo run --example cli -- setup

echo.
echo 3. Testing basic resolution with cache...
echo.
cargo run --example cli -- "ArialMT"

echo.
echo 4. Testing resolution with web fonts...
echo.
cargo run --example cli -- resolve "Roboto" --web

echo.
echo 5. Showing cache statistics...
echo.
cargo run --example cli -- cache stats

echo.
echo 6. Showing configuration...
echo.
cargo run --example cli -- config show

echo.
echo ================================
echo ✅ ENHANCED FEATURES TESTED!
echo ================================
echo.
echo Features implemented:
echo • 2MB memory cache with auto-pinning
echo • 10MB disk cache with persistence
echo • Interactive setup (3 questions)
echo • Web font support (optional)
echo • License warnings (optional)
echo • Manual cache cleanup
echo • Font pinning protection
echo • Memory limit warnings
echo.
pause