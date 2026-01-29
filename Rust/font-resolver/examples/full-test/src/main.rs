use font_normalizer::FontNormalizer;
use font_scanner::FontScanner;

fn main() {
    println!("=== Font Resolver Full Test ===\n");
    
    // Test 1: Normalizer
    println!("1. Testing Font Normalizer:");
    let normalizer = FontNormalizer;
    let test_fonts = vec![
        "ABCDEE+OpenSans-Bold",
        "ArialMT",
        "TimesNewRomanPS-BoldItalic",
        "Calibri-Light-Identity-H",
    ];
    
    for font_name in test_fonts {
        match normalizer.normalize(font_name) {
            Ok(request) => {
                println!("   '{}' → Family: {}, Weight: {}, Italic: {}",
                    font_name, request.family, request.weight, request.italic);
            }
            Err(e) => {
                println!("   Error normalizing '{}': {}", font_name, e);
            }
        }
    }
    
    // Test 2: Scanner
    println!("\n2. Testing Font Scanner:");
    let scanner = FontScanner;
    
    match scanner.scan_system_fonts() {
        Ok(fonts) => {
            println!("   Found {} system fonts", fonts.len());
            
            if !fonts.is_empty() {
                // Show first few fonts
                println!("\n   Sample fonts found (first 5):");
                for (i, font) in fonts.iter().take(5).enumerate() {
                    println!("     {}. {} (weight: {}, italic: {})",
                        i + 1, font.family, font.weight, font.italic);
                }
                
                if fonts.len() > 5 {
                    println!("     ... and {} more", fonts.len() - 5);
                }
                
                // Try to find specific fonts
                println!("\n   Searching for specific fonts:");
                let common_families = vec!["Arial", "Times", "Courier", "Calibri"];
                for family in common_families {
                    match scanner.find_font_by_family(family) {
                        Ok(Some(font)) => {
                            println!("     ✓ Found {}: {} (postscript: {})",
                                family, font.family, font.postscript_name);
                        }
                        Ok(None) => {
                            println!("     ✗ {} not found", family);
                        }
                        Err(e) => {
                            println!("     ! Error searching for {}: {}", family, e);
                        }
                    }
                }
            } else {
                println!("   No fonts found (scanner returned empty list)");
            }
        }
        Err(e) => {
            println!("   Error scanning fonts: {}", e);
        }
    }
    
    println!("\n=== Test Complete ===");
}