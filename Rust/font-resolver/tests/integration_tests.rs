// tests/integration_tests.rs
use font_resolver_engine::FontResolver;
use font_core::ResolverConfig;
use font_normalizer::FontNormalizer;
use font_scanner::FontScanner;

#[test]
fn test_normalizer_comprehensive() {
    let normalizer = FontNormalizer;
    
    let test_cases = vec![
        ("ABCDEE+OpenSans-Bold", "opensans", 700, false),
        ("ArialMT", "arial", 400, false),
        ("TimesNewRomanPS-BoldItalic", "times-new-roman", 700, true),
        ("Calibri-Light-Identity-H", "calibri", 300, false),
        ("Helvetica-Neue-Light", "helvetica-neue", 300, false),
        ("CourierNewPSMT", "courier-new", 400, false),
        ("Wingdings-Regular", "wingdings", 400, false),
        ("SymbolMT", "symbol", 400, false),
    ];
    
    for (input, expected_family, expected_weight, expected_italic) in test_cases {
        let result = normalizer.normalize(input).unwrap();
        assert_eq!(result.family, expected_family, "Failed for: {}", input);
        assert_eq!(result.weight, expected_weight, "Failed weight for: {}", input);
        assert_eq!(result.italic, expected_italic, "Failed italic for: {}", input);
    }
}

#[test]
fn test_scanner_finds_fonts() {
    let scanner = FontScanner;
    
    // This test will only work if there are fonts on the system
    match scanner.scan_system_fonts() {
        Ok(fonts) => {
            println!("Found {} fonts", fonts.len());
            assert!(!fonts.is_empty(), "Should find at least some fonts");
            
            // Check that we can find common fonts
            let common_families = ["Arial", "Times", "Courier", "Calibri"];
            for family in common_families {
                let found = scanner.find_fonts_by_family(family).unwrap();
                if !found.is_empty() {
                    println!("Found {} variants of {}", found.len(), family);
                }
            }
        }
        Err(e) => {
            // On CI or systems without fonts, this might fail
            println!("Scanner error (might be expected): {}", e);
        }
    }
}

#[test]
fn test_resolver_basic() {
    let config = ResolverConfig::default();
    let resolver = FontResolver::new(config);
    
    // Test resolution of known fonts
    let test_fonts = vec!["ArialMT", "TimesNewRomanPS-BoldItalic"];
    
    for font_name in test_fonts {
        match resolver.resolve(font_name) {
            Ok(result) => {
                println!("Resolved {} to {}", font_name, result.font.family);
                assert!(!result.font.family.is_empty());
            }
            Err(e) => {
                // This might fail if fonts aren't installed
                println!("Could not resolve {}: {}", font_name, e);
            }
        }
    }
}

#[test]
fn test_resolver_substitution() {
    let mut config = ResolverConfig::default();
    config.allow_substitution = true;
    config.preferred_families = vec!["Arial".to_string(), "Times New Roman".to_string()];
    
    let resolver = FontResolver::new(config);
    
    // Test a font that likely doesn't exist
    match resolver.resolve("NonexistentFont-123") {
        Ok(result) => {
            // Should be substituted
            assert!(result.substituted);
            assert!(result.compatibility_score > 0.0);
            println!("Substituted to: {}", result.font.family);
        }
        Err(_) => {
            // Might fail if no fonts available
            println!("No substitution available");
        }
    }
}