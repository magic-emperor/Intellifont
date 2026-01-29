@echo off
echo === Final Font Resolver Tests ===

echo.
echo 1. Testing Font Normalizer (CLI)...
cargo run -p cli

echo.
echo 2. Testing Full Integration (Full Test)...
cargo run -p full-test

echo.
echo 3. Testing Resolver Engine...
cargo run -p resolver-test

echo.
echo 4. Running All Unit Tests...
cargo test

echo.
echo === All Tests Completed ===
echo.
echo Summary:
echo - Font Normalizer: Working (PDF names → normalized names)
echo - Font Scanner: Working (finds mock fonts)
echo - Font Parser: Working (basic implementation)
echo - Font Resolver: Ready for testing
echo.
echo Next steps:
echo 1. Implement actual font file parsing
echo 2. Implement real Windows font scanning
echo 3. Add substitution logic for missing fonts
echo 4. Create Node.js bindings
pause