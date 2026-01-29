# Comprehensive test script for font-resolver CLI
# Tests all commands to ensure they work correctly

Write-Host "🧪 Testing Font Resolver CLI Commands" -ForegroundColor Cyan
Write-Host "=" * 60

$errors = 0
$passed = 0

function Test-Command {
    param(
        [string]$Name,
        [string]$Command,
        [int]$TimeoutSeconds = 30
    )
    
    Write-Host "`n📋 Testing: $Name" -ForegroundColor Yellow
    Write-Host "   Command: $Command" -ForegroundColor Gray
    
    $startTime = Get-Date
    try {
        $result = Invoke-Expression $Command 2>&1
        $duration = ((Get-Date) - $startTime).TotalSeconds
        
        if ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq $null) {
            Write-Host "   ✅ PASSED (${duration}s)" -ForegroundColor Green
            $script:passed++
            return $true
        } else {
            Write-Host "   ❌ FAILED (exit code: $LASTEXITCODE)" -ForegroundColor Red
            Write-Host "   Output: $($result -join "`n")" -ForegroundColor Gray
            $script:errors++
            return $false
        }
    } catch {
        Write-Host "   ❌ FAILED: $_" -ForegroundColor Red
        $script:errors++
        return $false
    }
}

# Test 1: Version
Test-Command "Version" 'cargo run -p font-resolver-cli -- --version'

# Test 2: Help
Test-Command "Help" 'cargo run -p font-resolver-cli -- --help'

# Test 3: Resolve basic font
Test-Command "Resolve Arial" 'cargo run -p font-resolver-cli -- resolve "Arial"'

# Test 4: Resolve with web fonts
Test-Command "Resolve with --web" 'cargo run -p font-resolver-cli -- resolve "Roboto" --web'

# Test 5: Tiered matching
Test-Command "Tiered Helvetica" 'cargo run -p font-resolver-cli -- tiered "Helvetica"'

# Test 6: Tiered with internet
Test-Command "Tiered with --internet" 'cargo run -p font-resolver-cli -- tiered "Helvetica" --internet'

# Test 7: Cache stats (should be fast now)
Test-Command "Cache Stats" 'cargo run -p font-resolver-cli -- cache stats'

# Test 8: Cache list
Test-Command "Cache List" 'cargo run -p font-resolver-cli -- cache list'

# Test 9: Cache suggest
Test-Command "Cache Suggest" 'cargo run -p font-resolver-cli -- cache suggest'

# Test 10: Config show
Test-Command "Config Show" 'cargo run -p font-resolver-cli -- config show'

# Test 11: Stats
Test-Command "Stats" 'cargo run -p font-resolver-cli -- stats'

# Test 12: Scan
Test-Command "Scan" 'cargo run -p font-resolver-cli -- scan'

# Test 13: Update database
Test-Command "Update Database" 'cargo run -p font-resolver-cli -- update'

# Summary
Write-Host "`n" + ("=" * 60)
Write-Host "📊 Test Summary" -ForegroundColor Cyan
Write-Host "   ✅ Passed: $passed" -ForegroundColor Green
Write-Host "   ❌ Failed: $errors" -ForegroundColor $(if ($errors -eq 0) { "Green" } else { "Red" })
Write-Host "=" * 60

if ($errors -eq 0) {
    Write-Host "`n🎉 All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "`n⚠️  Some tests failed. Please review the output above." -ForegroundColor Yellow
    exit 1
}
