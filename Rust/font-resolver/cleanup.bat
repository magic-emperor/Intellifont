@echo off
echo === Cleaning Up Structure ===

echo.
echo 1. Fixing examples/cli/src/main.rs...
(
echo use font_normalizer::FontNormalizer;
echo.
echo fn main() {
echo     println!("Font Resolver CLI - Testing Normalizer");
echo.
echo     let normalizer = FontNormalizer;
echo     let test_cases = vec![
echo         "ABCDEE+OpenSans-Bold",
echo         "ArialMT",
echo         "TimesNewRomanPS-BoldItalic",
echo         "Calibri-Light-Identity-H",
echo     ];
echo.
echo     for case in test_cases {
echo         match normalizer.normalize(case) {
echo             Ok(request) => {
echo                 println!("Original: {}", case);
echo                 echo "  Normalized: {}", request.normalized_name);
echo                 echo "  Family: {}", request.family);
echo                 echo "  Weight: {}", request.weight);
echo                 echo "  Italic: {}", request.italic);
echo                 echo;
echo             }
echo             Err(e) => {
echo                 echo "Error normalizing {}: {}", case, e);
echo             }
echo         }
echo     }
echo }
) > examples\cli\src\main.rs

echo.
echo 2. Fixing examples/cli/Cargo.toml...
(
echo [package]
echo name = "cli"
echo version = "0.1.0"
echo edition = "2021"
echo.
echo [dependencies]
echo font-normalizer = { path = "../../crates/font-normalizer" }
) > examples\cli\Cargo.toml

echo.
echo 3. Checking font-scanner is in the right place...
echo    FontScanner code should be in crates/font-scanner/src/lib.rs

echo.
echo === Cleanup Complete ===
echo Now run: cargo build