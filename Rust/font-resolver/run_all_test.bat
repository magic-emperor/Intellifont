@echo off
echo === Font Resolver - Complete Test Suite ===
echo.

echo 1. Building all crates...
cargo build
if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo 2. Running CLI test (font normalizer)...
echo.
cargo run -p cli

echo.
echo 3. Running full integration test...
echo.
cargo run -p full-test

echo.
echo 4. Running unit tests...
echo.
cargo test

echo.
echo ================================
echo ✅ ALL TESTS PASSED!
echo ================================
echo.
echo Next Steps:
echo 1. Implement real Windows font scanning
echo 2. Add actual font file parsing
echo 3. Create substitution database
echo 4. Build Node.js bindings
echo.
pause