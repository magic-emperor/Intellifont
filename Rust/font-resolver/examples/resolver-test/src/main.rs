use font_resolver_engine::FontResolver;
use font_core::ResolverConfig;

fn main() {
    println!("=== Font Resolver Engine Test ===\n");
    
    let config = ResolverConfig::default();
    let resolver = FontResolver::new(config);
    
    let test_fonts = vec![
        "ArialMT",
        "TimesNewRomanPS-BoldItalic",
        "Calibri-Light-Identity-H",
        "Helvetica",
        "NonexistentFont",
    ];
    
    for font_name in test_fonts {
        println!("Resolving: '{}'", font_name);
        
        match resolver.resolve(font_name) {
            Ok(result) => {
                println!("  ✓ Found: {}", result.font.family);
                println!("    Source: {:?}", result.source);
                println!("    Substituted: {}", result.substituted);
                if let Some(reason) = result.substitution_reason {
                    println!("    Reason: {:?}", reason);
                }
                if !result.warnings.is_empty() {
                    for warning in &result.warnings {
                        println!("    Warning: {}", warning);
                    }
                }
            }
            Err(e) => {
                println!("  ✗ Error: {}", e);
            }
        }
        println!();
    }
    
    // Test batch resolution
    println!("=== Batch Resolution Test ===");
    match resolver.resolve_batch(&["ArialMT", "TimesNewRomanPS-BoldItalic", "FakeFont"]) {
        Ok(results) => {
            for (i, result) in results.iter().enumerate() {
                println!("{}. {} -> {} (substituted: {})", 
                    i + 1,
                    result.original_name, 
                    result.font.family,
                    result.substituted);
            }
        }
        Err(e) => println!("Batch error: {}", e),
    }
}