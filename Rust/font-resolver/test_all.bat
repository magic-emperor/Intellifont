@echo off
echo === Testing Font Resolver ===

echo.
echo 1. Building workspace...
cargo build

if %errorlevel% neq 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo 2. Testing font normalizer (cli example)...
cargo run -p cli

if %errorlevel% neq 0 (
    echo CLI test failed!
    pause
    exit /b 1
)

echo.
echo 3. Testing full functionality (full-test example)...
cargo run -p full-test

echo.
echo 4. Running all tests...
cargo test

echo.
echo === All tests completed ===
pause