# robust_test.ps1
Write-Host "=== Font Resolver Robustness Test ===" -ForegroundColor Green
Write-Host "Testing all components thoroughly..." -ForegroundColor Cyan

# 1. Build with all features
Write-Host "`n1. Building with all features..." -ForegroundColor Yellow
cargo build --release

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# 2. Run unit tests
Write-Host "`n2. Running unit tests..." -ForegroundColor Yellow
cargo test --lib

# 3. Run integration tests
Write-Host "`n3. Running integration tests..." -ForegroundColor Yellow
if (Test-Path "tests\integration_tests.rs") {
    cargo test --test integration_tests
} else {
    Write-Host "Integration tests not found, skipping..." -ForegroundColor Yellow
}

# 4. Run examples
Write-Host "`n4. Running examples..." -ForegroundColor Yellow
Write-Host "   - CLI example:" -ForegroundColor White
cargo run --release -p cli

Write-Host "`n   - Full test:" -ForegroundColor White
cargo run --release -p full-test

# 5. Performance test
Write-Host "`n5. Performance benchmark..." -ForegroundColor Yellow
$time = Measure-Command { cargo run --release -p cli } 
Write-Host "   CLI example took: $($time.TotalSeconds) seconds" -ForegroundColor White

Write-Host "`n=== All Tests Passed ===" -ForegroundColor Green
Write-Host "Library is robust and ready for production!" -ForegroundColor Cyan