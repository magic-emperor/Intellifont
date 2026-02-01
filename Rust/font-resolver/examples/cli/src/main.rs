use clap::{Parser, Subcommand, ValueEnum};
use font_core::{EnhancedResolverConfig, FontError, ResolutionResult, FontDescriptor, FontFormat, LicenseInfo, FontMetrics};
use font_resolver_engine::{EnhancedFontResolver, TieredResolutionResult};
use font_setup::{apply_setup, interactive_setup, load_config, save_config, show_current_config};
use font_compressor::FontCompressor;
use std::process;
use std::path::PathBuf;
use std::fs;
use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use colored::*;
use tiny_http;
use serde_json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    // Handle version command
    if let Some(Commands::Version) = &cli.command {
        println!("{}", "=".repeat(60));
        println!("🎨 intelliFont Engine CLI v{}", env!("CARGO_PKG_VERSION"));
        println!("{}", "=".repeat(60));
        println!("Box: Premium font matching and suggestion engine");
        println!("🎯 Features: Tiered matching, Internet search, Metric substitution");
        println!("💾 Memory: 4.5MB optimized database, configurable cache");
        println!("🌐 Sources: Local System + Web (Google, Fontsource)");
        println!("⚖️  License: Proprietary / Enterprise Ready");
        return Ok(());
    }

    // Ensure database exists for commands that need it
    if !matches!(&cli.command, Some(Commands::Update) | Some(Commands::Setup)) {
        if let Err(e) = ensure_database_exists() {
            eprintln!("⚠️  Could not create database: {}", e);
            eprintln!("   Continuing with system fonts only...");
        }
    }
    
    // Handle font name as positional argument (shortcut) - only if no command is specified
    if let Some(font_name) = cli.font_name {
        if cli.command.is_some() {
            // This shouldn't happen due to clap's parsing, but handle it gracefully
            eprintln!("❌ Error: Cannot specify both a command and a font name as positional argument");
            eprintln!("   Usage: fr <font-name>  OR  fr <command> <args>");
            eprintln!("   Example: fr \"Arial\"  OR  fr tiered \"Helvetica\"");
            return Ok(());
        }
        let mut config = load_config()?;
        
        // Apply CLI flags
        if cli.no_cache {
            config.cache_enabled = false;
        }
        if cli.use_web_fonts {
            config.web_fonts_enabled = true;
        }
        
        // Try to load compressed database
        let database_path = PathBuf::from("data/font_database.bin");
        let resolver = if database_path.exists() {
            match load_or_create_database(&config) {
                Ok(resolver) => resolver,
                Err(_) => EnhancedFontResolver::new(config)?,
            }
        } else {
            EnhancedFontResolver::new(config)?
        };
        
        match resolver.resolve_font(&font_name) {
            Ok(result) => {
                print_resolution_result(&result, false);
                
                // Suggest web fonts if not found
                if result.substituted && !cli.use_web_fonts {
                    println!("💡 Tip: Enable web fonts with: fr resolve {} --web", font_name);
                }
            }
            Err(e) => {
                println!("❌ Error: {}", e);
                
                // Friendly suggestions based on error
                match e {
                    FontError::NotFound(_) => {
                        println!("💡 Suggestions:");
                        println!("   1. Enable web fonts: fr resolve {} --web", font_name);
                        println!("   2. Try tiered matching: fr tiered {}", font_name);
                        println!("   3. Check spelling or try similar: fr find-similar {}", font_name);
                    }
                    FontError::MemoryLimitExceeded(used, limit) => {
                        println!("💡 Memory limit exceeded: {:.1}MB > {}MB", used, limit);
                        println!("   Run: fr cache cleanup --aggressive");
                        println!("   Or increase limit: fr config set memory_limit {}", limit + 1);
                    }
                    _ => {}
                }
            }
        }
        return Ok(());
    }
    
    match cli.command {
        Some(Commands::Resolve { font_name, use_web_fonts, no_cache, detailed }) => {
            let mut config = load_config()?;
            
            // Apply command flags
            if no_cache {
                config.cache_enabled = false;
            }
            if use_web_fonts {
                config.web_fonts_enabled = true;
            }
            
            // **FIX: Use the basic FontResolver instead of EnhancedFontResolver**
            let basic_resolver = font_resolver_engine::FontResolver::new(
                font_core::ResolverConfig::default()
            );
            
            match basic_resolver.resolve(&font_name) {
                Ok(result) => {
                    print_resolution_result(&result, detailed);
                }
                Err(e) => {
                    println!("❌ Error: {}", e);
                    
                    // Try to load database manually
                    println!("🔍 Attempting to load database...");
                    let database_path = PathBuf::from("data/font_database.bin");
                    
                    if database_path.exists() {
                        println!("📁 Found database at: {:?}", database_path);
                        let database_data = fs::read(&database_path)?;
                        println!("📊 Database size: {} bytes", database_data.len());
                        
                        // Try with database
                        let compressor = font_compressor::FontCompressor::new(11, true);
                        match compressor.decompress_font_database(&database_data) {
                            Ok(db) => {
                                println!("✅ Successfully decompressed database with {} fonts", db.metadata.font_count);
                                
                                // Check if font is in database
                                for font in &db.fonts {
                                    if font.family.to_lowercase() == font_name.to_lowercase() {
                                        println!("🎯 Found in database: {}", font.family);
                                        println!("   Weight: {}, Italic: {}", font.weight, font.italic);
                                        return Ok(());
                                    }
                                }
                                println!("❌ Font not found in database");
                            }
                            Err(e) => println!("❌ Failed to decompress: {}", e),
                        }
                    } else {
                        println!("❌ Database not found at: {:?}", database_path);
                        println!("💡 Try running: cargo run --package font-resolver-cli -- update");
                    }
                }
            }
        }
        
        Some(Commands::TieredResolve { font_name, enable_internet }) => {
            println!("🎯 Tiered Matching for: '{}'", font_name.bold());
            println!("{}", "-".repeat(50));
            
            let mut config = load_config()?;
            config.web_fonts_enabled = true; // Enable web fonts for tiered matching
            
            // Try to load or create database
            let resolver = load_or_create_database(&config)?;
            
            // Show progress
            let pb = ProgressBar::new_spinner();
            pb.set_style(ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner} {msg}")?);
            pb.set_message("Analyzing font similarity...");
            pb.enable_steady_tick(Duration::from_millis(100));
            
            match resolver.resolve_with_tiered_matching(&font_name, enable_internet).await {
                Ok(result) => {
                    pb.finish_with_message("✓ Analysis complete");
                    
                    match result {
                        TieredResolutionResult::Exact(font, score) => {
                            println!("{}", "✅ EXACT MATCH FOUND".green().bold());
                            println!("   Font: {}", font.family.bold());
                            println!("   Similarity: {:.1}% (90-100% range)", score * 100.0);
                            println!("   Weight: {}, Italic: {}", font.weight, font.italic);
                            println!("   Source: System font");
                            
                            if let Some(license) = &font.license {
                                println!("   License: {}", 
                                    if license.allows_commercial_use { "✅ Free for commercial use".green() }
                                    else { "⚠️  Check license".yellow() }
                                );
                            }
                        }
                        TieredResolutionResult::Similar(fonts, score) => {
                            println!("{}", "🔍 SIMILAR FONTS FOUND".yellow().bold());
                            println!("   Best similarity: {:.1}% (80-90% range)", score * 100.0);
                            println!("   Top {} alternatives:", fonts.len().min(3));
                            
                            for (i, font) in fonts.iter().enumerate().take(3) {
                                println!("   {}. {} (weight: {}, italic: {})", 
                                        i + 1, font.family, font.weight, font.italic);
                            }
                            
                            if enable_internet {
                                println!("💡 Tip: Similarity below 90%. Enable internet search for exact match.");
                            }
                        }
                        TieredResolutionResult::SuggestInternet => {
                            println!("{}", "🌐 INTERNET SEARCH RECOMMENDED".blue().bold());
                            println!("   No good local matches found (similarity < 80%)");
                            println!("   To search online:");
                            println!("     1. Enable internet: cargo run -p font-resolver-cli -- tiered {} --internet", font_name);
                            println!("     2. Or use: cargo run -p font-resolver-cli -- resolve {} --web", font_name);
                            
                            if let Some(db_stats) = resolver.get_database_stats() {
                                println!("\n📦 Local database has {} fonts", db_stats.font_count);
                                println!("   Consider updating: fr update");
                            }
                        }
                        TieredResolutionResult::NotFound => {
                            println!("{}", "❌ NO MATCH FOUND".red().bold());
                            println!("   Font '{}' not found in any source", font_name);
                            println!("   Try:");
                            println!("     1. Check spelling");
                            println!("     2. Enable internet search: cargo run -p font-resolver-cli -- tiered {} --internet", font_name);
                            println!("     3. Update font database: fr update");
                        }
                    }
                }
                Err(e) => {
                    pb.finish_with_message("✗ Analysis failed");
                    println!("❌ Error: {}", e);
                }
            }
        }
        
        Some(Commands::Setup) => {
            println!("{}", "=".repeat(60));
            println!("🎨 FONT RESOLVER - INTERACTIVE SETUP");
            println!("{}", "=".repeat(60));
            
            let setup_config = interactive_setup();
            let config = apply_setup(&setup_config);
            
            save_config(&config)?;
            
            println!("\n{}", "✅ SETUP COMPLETED".green().bold());
            println!("📁 Configuration saved to: {:?}", font_setup::get_config_path()?);
            
            // Create initial compressed database if not exists
            let database_path = PathBuf::from("data/font_database.bin");
            if !database_path.exists() {
                println!("📦 Creating initial font database...");
                ensure_database_exists()?;
            }
            
            println!("\n{}", "🚀 QUICK START".bold());
            println!("   Test basic resolution: {}", "fr resolve \"Arial\"".cyan());
            println!("   Test tiered matching: {}", "fr tiered \"Helvetica\"".cyan());
            println!("   Update fonts: {}", "fr update".cyan());
            
            handle_memory_limit(&mut config.clone());
        }
        
        Some(Commands::Cache(cmd)) => match cmd {
            CacheCommands::Stats => {
                println!("📊 Loading cache statistics...");
                let config = load_config()?;
                
                // Use a timeout or make this non-blocking
                let resolver = match std::panic::catch_unwind(|| {
                    EnhancedFontResolver::new(config)
                }) {
                    Ok(Ok(resolver)) => resolver,
                    Ok(Err(e)) => {
                        println!("❌ Error initializing resolver: {}", e);
                        println!("💡 Tip: Try running 'fr cache cleanup' to free up space");
                        return Ok(());
                    }
                    Err(_) => {
                        println!("❌ Panic during resolver initialization");
                        println!("💡 Tip: Cache may be corrupted, try 'fr cache cleanup --aggressive'");
                        return Ok(());
                    }
                };
                
                println!("✅ Resolver initialized, fetching stats...");
                
                // Use a spinner to show progress
                let pb = ProgressBar::new_spinner();
                pb.set_style(ProgressStyle::default_spinner()
                    .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                    .template("{spinner} {msg}")?);
                pb.set_message("Calculating cache statistics...");
                pb.enable_steady_tick(Duration::from_millis(100));
                
                let stats_result = resolver.get_cache_stats();
                pb.finish_with_message("✓ Stats calculated");
                
                match stats_result {
                    Some(stats_result) => match stats_result {
                        Ok(stats) => {
                            println!("{}", "📊 CACHE STATISTICS".bold());
                            println!("{}", "-".repeat(40));
                            println!("   Memory entries: {}", stats.memory_entries);
                            println!("   Disk entries: {}", stats.disk_entries);
                            println!("   Pinned fonts: {}", stats.pinned_fonts);
                            println!("   Memory usage: {:.2}MB / {}MB", 
                                    stats.memory_usage_mb, resolver.get_config().memory_limit_mb);
                            
                            // Show note if disk usage is estimated
                            if stats.disk_entries >= 100 {
                                println!("   Disk usage: ~{:.2}MB / {}MB (estimated)", 
                                        stats.disk_usage_mb, resolver.get_config().disk_limit_mb);
                                println!("   {} Note: Disk usage is estimated for large caches", "ℹ️".blue());
                            } else {
                                println!("   Disk usage: {:.2}MB / {}MB", 
                                        stats.disk_usage_mb, resolver.get_config().disk_limit_mb);
                            }
                            
                            let memory_percent = (stats.memory_usage_mb / resolver.get_config().memory_limit_mb as f64) * 100.0;
                            let disk_percent = (stats.disk_usage_mb / resolver.get_config().disk_limit_mb as f64) * 100.0;
                            
                            if memory_percent > 80.0 {
                                println!("{}", "⚠️  WARNING: Memory cache is >80% full".yellow());
                                println!("   Run: {} to free space", "fr cache cleanup --aggressive".cyan());
                            }
                            if disk_percent > 80.0 {
                                println!("{}", "⚠️  WARNING: Disk cache is >80% full".yellow());
                                println!("   Run: {} to clean up", "fr cache cleanup".cyan());
                            }
                            
                            if memory_percent < 30.0 && disk_percent < 30.0 {
                                println!("{}", "✅ Cache is healthy".green());
                            }
                        }
                        Err(e) => {
                            println!("❌ Error getting cache stats: {}", e);
                            println!("💡 Tip: Try running 'fr cache cleanup' to fix cache issues");
                        }
                    },
                    None => println!("{}", "⚠️  Cache is disabled".yellow()),
                }
            }
            
            CacheCommands::Cleanup { aggressive, dry_run } => {
                let config = load_config()?;
                let resolver = EnhancedFontResolver::new(config)?;
                
                if dry_run {
                    match resolver.suggest_cleanup() {
                        Some(suggestions_result) => match suggestions_result {
                            Ok(suggestions) => {
                                if suggestions.is_empty() {
                                    println!("✅ Nothing to clean up!");
                                } else {
                                    println!("🔍 Would remove {} items:", suggestions.len());
                                    for suggestion in &suggestions {
                                        println!("   {}", suggestion);
                                    }
                                    println!("\nRun without --dry-run to execute cleanup");
                                    println!("Command: {}", "fr cache cleanup".cyan());
                                }
                            }
                            Err(e) => println!("❌ Error: {}", e),
                        },
                        None => println!("⚠️  Cache is disabled"),
                    }
                } else {
                    let pb = ProgressBar::new_spinner();
                    pb.set_style(ProgressStyle::default_spinner()
                        .template("{spinner} {msg}")?);
                    pb.set_message("Cleaning up cache...");
                    pb.enable_steady_tick(Duration::from_millis(100));
                    
                    match resolver.cleanup_cache(aggressive) {
                        Ok(removed) => {
                            pb.finish_with_message("✓ Cleanup complete");
                            println!("✅ Removed {} entries from cache", removed);
                        }
                        Err(e) => {
                            pb.finish_with_message("✗ Cleanup failed");
                            println!("❌ Error: {}", e);
                        }
                    }
                }
            }
            
            CacheCommands::Pin { font_name } => {
                let config = load_config()?;
                let resolver = EnhancedFontResolver::new(config)?;
                
                match resolver.pin_font(&font_name) {
                    Ok(_) => println!("✅ Pinned '{}' (will never be deleted)", font_name),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            
            CacheCommands::Unpin { font_name } => {
                let config = load_config()?;
                let resolver = EnhancedFontResolver::new(config)?;
                
                match resolver.unpin_font(&font_name) {
                    Ok(_) => println!("✅ Unpinned '{}'", font_name),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            
            CacheCommands::List => {
                let config = load_config()?;
                let resolver = EnhancedFontResolver::new(config)?;
                
                match resolver.list_pinned_fonts() {
                    Some(pinned) => {
                        if pinned.is_empty() {
                            println!("📌 No fonts are pinned");
                            println!("   Auto-pinning happens after 5 uses");
                            println!("   Or manually pin: {}", "fr cache pin <font-name>".cyan());
                        } else {
                            println!("📌 Pinned fonts ({}):", pinned.len());
                            for font in pinned {
                                println!("   • {}", font);
                            }
                        }
                    }
                    None => println!("⚠️  Cache is disabled"),
                }
            }
            
            CacheCommands::Suggest => {
                let config = load_config()?;
                let resolver = EnhancedFontResolver::new(config)?;
                
                match resolver.suggest_cleanup() {
                    Some(suggestions_result) => match suggestions_result {
                        Ok(suggestions) => {
                            if suggestions.is_empty() {
                                println!("✅ Cache is clean! Nothing to suggest for cleanup.");
                            } else {
                                println!("🔍 Cleanup suggestions ({} items):", suggestions.len());
                                for suggestion in &suggestions {
                                    println!("   {}", suggestion);
                                }
                                println!("\nRun: {}", "fr cache cleanup".cyan());
                            }
                        }
                        Err(e) => println!("❌ Error: {}", e),
                    },
                    None => println!("⚠️  Cache is disabled"),
                }
            }
        },
        
        Some(Commands::Config(subcommand)) => match subcommand {
            ConfigCommands::Show => {
                show_current_config()?;
            }
            
            ConfigCommands::Set { key, value } => {
                let mut config = load_config()?;
                
                match key.to_lowercase().as_str() {
                    "memory_limit" | "memory" => {
                        if let Ok(limit) = value.parse::<usize>() {
                            if limit < 1 {
                                println!("❌ Memory limit must be at least 1MB");
                                process::exit(1);
                            }
                            if limit > 1024 {
                                println!("⚠️  Warning: Memory limit above 1GB is not recommended");
                            }
                            config.memory_limit_mb = limit;
                            println!("✅ Memory limit set to {}MB", limit);
                        } else {
                            println!("❌ Invalid memory limit. Use a number like 2, 4, 8");
                        }
                    }
                    
                    "disk_limit" | "disk" => {
                        if let Ok(limit) = value.parse::<usize>() {
                            if limit < 10 {
                                println!("❌ Disk limit must be at least 10MB");
                                process::exit(1);
                            }
                            config.disk_limit_mb = limit;
                            println!("✅ Disk limit set to {}MB", limit);
                        } else {
                            println!("❌ Invalid disk limit. Use a number like 50, 100, 500");
                        }
                    }
                    
                    "web_fonts" | "web" => {
                        match value.to_lowercase().as_str() {
                            "true" | "yes" | "1" | "on" => {
                                config.web_fonts_enabled = true;
                                println!("✅ Web fonts enabled");
                            }
                            "false" | "no" | "0" | "off" => {
                                config.web_fonts_enabled = false;
                                println!("✅ Web fonts disabled");
                            }
                            _ => println!("❌ Use true/false, yes/no, or on/off"),
                        }
                    }
                    
                    "license_warnings" | "license" => {
                        match value.to_lowercase().as_str() {
                            "true" | "yes" | "1" | "on" => {
                                config.license_warnings = font_core::LicenseWarningLevel::All;
                                println!("✅ License warnings enabled");
                            }
                            "false" | "no" | "0" | "off" => {
                                config.license_warnings = font_core::LicenseWarningLevel::Off;
                                println!("✅ License warnings disabled");
                            }
                            _ => println!("❌ Use true/false, yes/no, or on/off"),
                        }
                    }
                    
                    "auto_pin" | "autopin" => {
                        match value.to_lowercase().as_str() {
                            "true" | "yes" | "1" | "on" => {
                                config.auto_pin_threshold = 5;
                                println!("✅ Auto-pinning enabled (after 5 uses)");
                            }
                            "false" | "no" | "0" | "off" => {
                                config.auto_pin_threshold = 0;
                                println!("✅ Auto-pinning disabled");
                            }
                            _ => {
                                if let Ok(threshold) = value.parse::<u32>() {
                                    config.auto_pin_threshold = threshold;
                                    println!("✅ Auto-pinning threshold set to {} uses", threshold);
                                } else {
                                    println!("❌ Use a number for threshold, or true/false");
                                }
                            }
                        }
                    }
                    
                    _ => {
                        println!("❌ Unknown configuration key: {}", key);
                        println!("   Available keys:");
                        println!("     - memory_limit: Set memory cache size (MB)");
                        println!("     - disk_limit: Set disk cache size (MB)");
                        println!("     - web_fonts: Enable/disable web fonts");
                        println!("     - license_warnings: Enable/disable license warnings");
                        println!("     - auto_pin: Enable/disable auto-pinning");
                        return Ok(());
                    }
                }
                
                save_config(&config)?;
                println!("   Configuration saved.");
            }
            
            ConfigCommands::Reset => {
                let default_config = EnhancedResolverConfig::default();
                save_config(&default_config)?;
                println!("✅ Configuration reset to defaults");
                show_current_config()?;
            }
            
            ConfigCommands::Export { path } => {
                let config = load_config()?;
                let config_str = toml::to_string_pretty(&config)?;
                std::fs::write(&path, config_str)?;
                println!("✅ Configuration exported to: {}", path);
            }
            
            ConfigCommands::Import { path } => {
                let config_str = std::fs::read_to_string(&path)?;
                let config: EnhancedResolverConfig = toml::from_str(&config_str)?;
                save_config(&config)?;
                println!("✅ Configuration imported from: {}", path);
                show_current_config()?;
            }
        },
        
        Some(Commands::Scan { detailed }) => {
            println!("🔍 Scanning system fonts...");
            
            use font_scanner::FontScanner;
            let scanner = FontScanner;
            
            match scanner.scan_system_fonts() {
                Ok(fonts) => {
                    println!("✅ Found {} system fonts", fonts.len());
                    
                    if detailed {
                        println!("\n{}", "DETAILED FONT LIST".bold());
                        println!("{}", "-".repeat(60));
                        
                        // Count by format
                        use std::collections::HashMap;
                        let mut format_counts = HashMap::new();
                        for font in &fonts {
                            *format_counts.entry(font.format.to_string()).or_insert(0) += 1;
                        }
                        
                        println!("Formats:");
                        for (format, count) in format_counts {
                            println!("  {}: {}", format, count);
                        }
                        
                        // Show first 10 fonts
                        println!("\nFirst 10 fonts:");
                        for font in fonts.iter().take(10) {
                            println!("  • {} ({}, weight: {})", 
                                    font.family, font.format, font.weight);
                        }
                    }
                }
                Err(e) => println!("❌ Scan failed: {}", e),
            }
        }
        
        Some(Commands::Stats) => {
            let config = load_config()?;
            
            // Try to load compressed database
            let resolver = load_or_create_database(&config)?;
            
            println!("{}", "📈 FONT RESOLVER STATISTICS".bold());
            println!("{}", "=".repeat(40));
            
            // Cache stats
            match resolver.get_cache_stats() {
                Some(stats_result) => match stats_result {
                    Ok(stats) => {
                        println!("{}", "💾 CACHE".bold());
                        println!("  Memory: {:.1}MB / {}MB", 
                                stats.memory_usage_mb, resolver.get_config().memory_limit_mb);
                        println!("  Disk: {:.1}MB / {}MB", 
                                stats.disk_usage_mb, resolver.get_config().disk_limit_mb);
                        println!("  Entries: {} memory, {} disk, {} pinned", 
                                stats.memory_entries, stats.disk_entries, stats.pinned_fonts);
                    }
                    Err(e) => println!("  Cache stats error: {}", e),
                },
                None => println!("  Cache: Disabled"),
            }
            
            // Database stats
            if let Some(db_stats) = resolver.get_database_stats() {
                println!("\n{}", "📦 COMPRESSED DATABASE".bold());
                println!("  Fonts: {}", db_stats.font_count);
                println!("  Size: {:.2}MB (compressed from {:.2}MB)", 
                        db_stats.compressed_size_mb, db_stats.original_size_mb);
                println!("  Compression: {:.1}%", db_stats.compression_ratio);
                
                if !db_stats.categories.is_empty() {
                    println!("  Categories:");
                    for (category, count) in &db_stats.categories {
                        println!("    • {:?}: {}", category, count);
                    }
                }
            } else {
                println!("\n{}", "📦 DATABASE".bold());
                println!("  No compressed database loaded");
                println!("  Run 'fr update' to download font database");
            }
            
            // Configuration info
            let config = resolver.get_config();
            println!("\n{}", "⚙️  CONFIGURATION".bold());
            println!("  Web fonts: {}", 
                    if config.web_fonts_enabled { "✅ Enabled".green() } 
                    else { "❌ Disabled".red() });
            println!("  License warnings: {}", config.license_warnings);
            println!("  Auto-pin threshold: {} uses", config.auto_pin_threshold);
            println!("  Memory limit: {}MB, Disk limit: {}MB", 
                    config.memory_limit_mb, config.disk_limit_mb);
        }
        
        Some(Commands::FindSimilar { font_name, limit:_limit }) => {
            println!("🔍 Finding fonts similar to '{}'...", font_name);
            
            // This is a placeholder - real implementation would use similarity engine
            println!("⚠️  This feature requires the font-similarity crate");
            println!("   For now, try: {}", format!("fr tiered {}", font_name).cyan());
            println!("   Or: {}", format!("fr resolve {} --web", font_name).cyan());
        }
        
        Some(Commands::CheckLicense { font_name }) => {
            println!("⚖️  Checking license for '{}'...", font_name);
            
            let config = load_config()?;
            let resolver = EnhancedFontResolver::new(config)?;
            
            match resolver.check_license(&font_name) {
                Ok(warning) => {
                    println!("{}", "LICENSE ANALYSIS".bold());
                    println!("{}", "-".repeat(40));
                    println!("Font: {}", font_name);
                    println!("License type: {:?}", warning.license_type);
                    println!("Warning level: {:?}", warning.warning_level);
                    println!("Message: {}", warning.message);
                    
                    if !warning.alternatives.is_empty() {
                        println!("\n{}", "FREE ALTERNATIVES".bold());
                        for alt in &warning.alternatives {
                            println!("  • {} ({:.0}% similar) - {}", 
                                    alt.family, alt.similarity_score * 100.0, alt.reason);
                        }
                    }
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }
        
        Some(Commands::Update) => {
            println!("{}", "🌐 UPDATING FONT DATABASE".bold());
            println!("{}", "=".repeat(40));
            
            println!("🔍 Creating compressed font database...");
            
            // Try to create a proper database
            match create_minimal_database() {
                Ok(compressed_data) => {
                    let config = EnhancedResolverConfig::default();
                    
                    match EnhancedFontResolver::new_with_database(config, &compressed_data) {
                        Ok(resolver) => {
                            println!("✅ Database created successfully!");
                            
                            // Save to file
                            let db_path = PathBuf::from("data/font_database.bin");
                            if let Some(parent) = db_path.parent() {
                                std::fs::create_dir_all(parent)?;
                            }
                            std::fs::write(&db_path, &compressed_data)?;
                            
                            if let Some(stats) = resolver.get_database_stats() {
                                println!("   Fonts: {}", stats.font_count);
                                println!("   Size: {:.2}MB", stats.compressed_size_mb);
                                println!("   Compression: {:.1}%", stats.compression_ratio);
                            }
                            
                            println!("\n📁 Database saved to: {}", db_path.display());
                            println!("   Test with: {}", "fr resolve \"Arial\"".cyan());
                            
                            // Offer to enable web fonts
                            println!("\n💡 Tip: Enable web fonts for better results:");
                            println!("   {}", "fr config set web_fonts true".cyan());
                        }
                        Err(e) => {
                            println!("❌ Failed to create resolver: {}", e);
                            println!("   Creating basic database file...");
                            
                            // Create a basic database file anyway
                            let db_path = PathBuf::from("data/font_database.bin");
                            std::fs::write(&db_path, b"BASIC_DATABASE_V1")?;
                            println!("✅ Created basic database file");
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to create database: {}", e);
                    println!("   Creating empty database file...");
                    
                    // Create at least an empty file
                    let db_path = PathBuf::from("data/font_database.bin");
                    std::fs::write(&db_path, b"EMPTY_DATABASE")?;
                    println!("✅ Created placeholder database file");
                }
            }
        }
        
        Some(Commands::Version) => {
            // Already handled at the beginning
        }
        
        Some(Commands::Identify { path, chars, db, verbose, json }) => {
            if !json {
                println!("👁️  IDENTIFYING FONT VISUALLY");
                println!("{}", "=".repeat(40));
            }
            
            let db_path = db.unwrap_or_else(|| PathBuf::from("data/glyph_signatures.bin"));
            
            if !db_path.exists() {
                if json {
                    println!("{{ \"error\": \"Glyph database not found\" }}");
                } else {
                    println!("❌ Glyph database not found at: {}", db_path.display());
                    println!("   Run 'fr build-glyph-db' to create it.");
                }
                return Ok(());
            }
            
            if !path.exists() {
                if json {
                    println!("{{ \"error\": \"Font file not found\" }}");
                } else {
                    println!("❌ Font file not found: {}", path.display());
                }
                return Ok(());
            }
            
            if !json {
                println!("📦 Loading database...");
            }
            let identifier = match font_visual_id::VisualIdentifier::from_file(&db_path) {
                Ok(id) => id,
                Err(e) => {
                    if json {
                        println!("{{ \"error\": \"Failed to load database: {}\" }}", e);
                    } else {
                        println!("❌ Failed to load database: {}", e);
                    }
                    return Ok(());
                }
            };

            if verbose && !json {
                println!("\n🔬 SIGNATURE ANALYSIS (The 'DNA' of the font)");
                println!("{}", "-".repeat(40));
                
                // Extract signature for first char to show details
                if let Some(first_char) = chars.chars().next() {
                    println!("Analyzing character: '{}'", first_char);
                     match identifier.extract_signature(&path, first_char) {
                         Ok(sig) => {
                             println!("  • Aspect Ratio: {:3} (Width/Height profile)", sig.aspect_ratio);
                             println!("  • Density:      {:3}/255 (Ink coverage)", sig.density);
                             println!("  • Stroke Width: {:3}/255 (Estimated weight)", sig.stroke_width);
                             println!("  • Curvature:    {:3}/255 (Curves vs Lines)", sig.curve_ratio);
                             println!("  • Points:       {:3}     (Complexity)", sig.point_count);
                             println!("  • Balance:      X={:3}, Y={:3} (Center of mass)", sig.x_balance, sig.y_balance);
                             println!("  • Serifs:       {:3}/255 (Serif detection)", sig.serif_score);
                             println!("  • LSH Hash:     0x{:04x} (Locality Bucket)", sig.feature_hash);
                         }
                         Err(e) => println!("  Failed to extract signature: {}", e),
                     }
                }
                println!("{}", "-".repeat(40));
            }
            
            if !json {
                println!("\n🔍 Analyzing font: {}", path.display());
                println!("   Characters: {}", chars);
            }
            
            match identifier.identify_multi(&path, &chars, 5) {
                Ok(results) => {
                    if json {
                         println!("{}", serde_json::to_string_pretty(&results).unwrap_or_default());
                    } else {
                        if results.is_empty() {
                            println!("❌ No matches found.");
                        } else {
                            println!("\n✅ MATCHES FOUND");
                            println!("{}", "-".repeat(40));
                            for (i, result) in results.iter().enumerate() {
                                // Simple manual color interpolation
                                let confidence = result.confidence * 100.0;
                                let score_str = format!("{:.1}%", confidence);
                                
                                let colored_score = if confidence > 90.0 {
                                    score_str.green().bold()
                                } else if confidence > 70.0 {
                                    score_str.yellow()
                                } else if confidence > 50.0 {
                                    score_str.yellow().dimmed()
                                } else {
                                    score_str.red()
                                };
                                
                                println!("{}. {} {} ({})", 
                                    i + 1, 
                                    result.family.bold(),
                                    result.subfamily.as_deref().unwrap_or("").dimmed(),
                                    colored_score
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    if json {
                        println!("{{ \"error\": \"Identification failed: {}\" }}", e);
                    } else {
                        println!("❌ Identification failed: {}", e);
                    }
                }
            }
        }

        Some(Commands::AiSuggest { path, limit, db, json }) => {
            if !json {
                println!("🤖 AI FONT SIMILARITY ANALYSIS");
                println!("{}", "=".repeat(40));
            }
            
            let db_path = db.unwrap_or_else(|| PathBuf::from("data/glyph_signatures.bin"));
            
            if !db_path.exists() {
                if json {
                    println!("{{ \"error\": \"AI model database not found\" }}");
                } else {
                    println!("❌ AI model database not found at: {}", db_path.display());
                    println!("   Run 'fr build-glyph-db' to create it.");
                }
                return Ok(());
            }
            
            if !path.exists() {
                if json {
                    println!("{{ \"error\": \"Font file not found\" }}");
                } else {
                    println!("❌ Font file not found: {}", path.display());
                }
                return Ok(());
            }
            
            if !json {
                println!("📦 Loading AI model...");
            }
            let identifier = match font_visual_id::VisualIdentifier::from_file(&db_path) {
                Ok(id) => id,
                Err(e) => {
                    if json {
                        println!("{{ \"error\": \"Failed to load AI model: {}\" }}", e);
                    } else {
                        println!("❌ Failed to load AI model: {}", e);
                    }
                    return Ok(());
                }
            };
            
            if !json {
                println!("🔍 Analyzing font: {}", path.display());
            }
            
            match identifier.identify_multi(&path, "RQWM", limit as usize) {
                Ok(results) => {
                    if json {
                        // Build JSON with match_quality
                        let suggestions: Vec<_> = results.iter().map(|r| {
                            let quality = if r.confidence >= 0.95 { "exact" }
                                else if r.confidence >= 0.85 { "high" }
                                else if r.confidence >= 0.70 { "medium" }
                                else { "low" };
                            serde_json::json!({
                                "family": r.family,
                                "subfamily": r.subfamily,
                                "confidence": r.confidence,
                                "match_quality": quality
                            })
                        }).collect();
                        println!("{}", serde_json::to_string_pretty(&suggestions).unwrap_or_default());
                    } else {
                        if results.is_empty() {
                            println!("❌ No similar fonts found.");
                        } else {
                            println!("\n✨ AI SUGGESTIONS");
                            println!("{}", "-".repeat(40));
                            for (i, result) in results.iter().enumerate() {
                                let confidence = result.confidence * 100.0;
                                let quality = if confidence > 95.0 { "EXACT".green().bold() }
                                    else if confidence > 85.0 { "HIGH".green() }
                                    else if confidence > 70.0 { "MEDIUM".yellow() }
                                    else { "LOW".red() };
                                
                                println!("{}. {} {} [{} - {:.1}%]", 
                                    i + 1, 
                                    result.family.bold(),
                                    result.subfamily.as_deref().unwrap_or("").dimmed(),
                                    quality,
                                    confidence
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    if json {
                        println!("{{ \"error\": \"AI analysis failed: {}\" }}", e);
                    } else {
                        println!("❌ AI analysis failed: {}", e);
                    }
                }
            }
        }

        Some(Commands::BuildWebDb { limit, output }) => {
            println!("🌐 BUILDING WEB FONT DATABASE");
            println!("{}", "=".repeat(40));
            
            use font_acquisition::{FontAcquisitionManager, FontsourceProvider};
            
            // Setup manager
            let mut manager = FontAcquisitionManager::new();
            manager.add_provider("fontsource", Box::new(FontsourceProvider::new()));
            
            println!("🔍 Searching for popular fonts...");
            // Search for "sans" and "serif" to get a mix
            let search_limit = limit / 2 + 1;
            let sans = manager.parallel_search("sans", search_limit).await.unwrap_or_default();
            let serif = manager.parallel_search("serif", search_limit).await.unwrap_or_default();
            
            let mut all_fonts = sans;
            all_fonts.extend(serif);
            all_fonts.truncate(limit);
            
            println!("   Found {} candidate fonts", all_fonts.len());
            
            if all_fonts.is_empty() {
                println!("❌ No fonts found. Check internet connection.");
                return Ok(());
            }

            // Setup temp dir in current directory
            let temp_dir = PathBuf::from("temp_web_fonts");
            if !temp_dir.exists() {
                std::fs::create_dir(&temp_dir)?;
            }
            
            println!("⬇️  Downloading and Indexing...");
            let pb = ProgressBar::new(all_fonts.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")?
                .progress_chars("=>-"));
                
            let mut builder = font_glyph_db::GlyphDatabaseBuilder::new();
            let mut success = 0;
            
            for font_data in all_fonts {
                pb.set_message(format!("{}", font_data.family));
                
                // Download
                match manager.download_and_verify(&font_data, font_core::FontFormat::Ttf, "Fontsource").await {
                    Ok(descriptor) => {
                        // Add to DB
                        match builder.add_font_auto(&descriptor.path) {
                            Ok(_) => success += 1,
                            Err(_) => {}
                        }
                    }
                    Err(_) => {
                         // Ignore download failures, common with free APIs
                    }
                }
                pb.inc(1);
            }
            pb.finish_with_message("Processing complete");
            
            println!("✅ Indexed {} fonts", success);
            
            println!("💾 Saving database to: {}", output.display());
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            match builder.save_to_file(&output) {
                Ok(stats) => {
                    println!("\n✅ WEB DATABASE BUILT SUCCESSFULLY");
                    println!("{}", stats);
                }
                Err(e) => println!("❌ Failed to save database: {}", e),
            }
            
            // Allow cleanup of temp dir (optional, user might want them?)
             println!("🧹 Cleaning up temporary files...");
             if let Err(e) = std::fs::remove_dir_all(&temp_dir) {
                 println!("⚠️  Failed to remove temp dir: {}", e);
             }
        }


        Some(Commands::BuildGlyphDb { source, output, compression: _, recursive }) => {
            println!("🏗️  BUILDING GLYPH DATABASE");
            println!("{}", "=".repeat(40));
            
            if !source.exists() {
                println!("❌ Source directory not found: {}", source.display());
                return Ok(());
            }
            
            println!("📂 Scanning fonts in: {}", source.display());
            let mut font_paths = Vec::new();
            
            // Simple recursive walker
            let mut dirs = vec![source];
            while let Some(dir) = dirs.pop() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if recursive {
                                dirs.push(path);
                            }
                        } else {
                            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                match ext.to_lowercase().as_str() {
                                    "ttf" | "otf" => font_paths.push(path),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            
            println!("   Found {} font files", font_paths.len());
            
            if font_paths.is_empty() {
                println!("⚠️  No fonts found. Exiting.");
                return Ok(());
            }
            
            println!("🔨 Indexing fonts (this may take a while)...");
            let pb = ProgressBar::new(font_paths.len() as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")?
                .progress_chars("=>-"));
                
            let mut builder = font_glyph_db::GlyphDatabaseBuilder::new();
            let mut success_count = 0;
            
            for path in &font_paths {
                pb.set_message(path.file_name().unwrap_or_default().to_string_lossy().to_string());
                match builder.add_font_auto(&path) {
                    Ok(_) => success_count += 1,
                    Err(_) => {} // Skip errors
                }
                pb.inc(1);
            }
            pb.finish_with_message("Indexing complete");
            println!("✅ Indexed {}/{} fonts", success_count, font_paths.len());
            
            println!("💾 Saving database to: {}", output.display());
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            match builder.save_to_file(&output) {
                Ok(stats) => {
                    println!("\n✅ DATABASE BUILT SUCCESSFULLY");
                    println!("{}", stats);
                }
                Err(e) => println!("❌ Failed to save database: {}", e),
            }
        }

        Some(Commands::GlyphDbStats { path }) => {
            println!("📊 GLYPH DATABASE STATISTICS");
            println!("{}", "=".repeat(40));
            
            if !path.exists() {
                println!("❌ Database file not found: {}", path.display());
                return Ok(());
            }
            
            match font_visual_id::VisualIdentifier::from_file(&path) {
                Ok(identifier) => {
                    let stats = identifier.database_stats();
                    println!("{}", stats);
                    
                    let metadata = std::fs::metadata(&path)?;
                    println!("  File size: {:.2}MB", metadata.len() as f64 / 1_000_000.0);
                }
                Err(e) => println!("❌ Failed to load database: {}", e),
            }
        }

        Some(Commands::Serve { port, host, db }) => {
            let server = match tiny_http::Server::http(format!("{}:{}", host, port)) {
                Ok(s) => s,
                Err(e) => {
                    println!("❌ Failed to start server: {}", e);
                    return Ok(());
                }
            };
            
            println!("🚀 intelliFont AI Microservice running on {}:{}", host, port);
            println!("📦 Using database: {}", db.display());
            println!("💡 Use POST /api/identify with raw binary font data to identify");
            println!("💡 Use POST /api/ai-suggest with raw binary font data for similarity");
            
            // Load database into memory once
            let identifier = match font_visual_id::VisualIdentifier::from_file(&db) {
                Ok(id) => id,
                Err(e) => {
                    println!("❌ Failed to load AI model database: {}", e);
                    return Ok(());
                }
            };

            for mut request in server.incoming_requests() {
                let response = match (request.method(), request.url()) {
                    (&tiny_http::Method::Get, "/health") => {
                        tiny_http::Response::from_string("{\"status\": \"ok\", \"engine\": \"intelliFont\"}")
                            .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                    },
                    (&tiny_http::Method::Post, "/api/identify") | (&tiny_http::Method::Post, "/api/ai-suggest") => {
                        let is_suggest = request.url().contains("ai-suggest");
                        
                        // Get binary data
                        let mut body = Vec::new();
                        let _ = request.as_reader().read_to_end(&mut body);
                        
                        if body.is_empty() {
                            tiny_http::Response::from_string("{\"error\": \"Empty body\"}")
                                .with_status_code(400)
                                .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                        } else {
                            // Temporary identifying
                            let tmp_id = format!("serve_{}.bin", chrono::Local::now().timestamp_nanos_opt().unwrap_or(0));
                            let tmp_path = std::env::temp_dir().join(tmp_id);
                            
                            if let Err(e) = std::fs::write(&tmp_path, body) {
                                tiny_http::Response::from_string(format!("{{\"error\": \"FS error: {}\"}}", e))
                                    .with_status_code(500)
                                    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                            } else {
                                let results = if is_suggest {
                                    match identifier.identify_multi(&tmp_path, "RQWM", 5) {
                                        Ok(res) => {
                                            let suggestions: Vec<_> = res.iter().map(|r| {
                                                let quality = if r.confidence >= 0.95 { "exact" }
                                                    else if r.confidence >= 0.85 { "high" }
                                                    else if r.confidence >= 0.70 { "medium" }
                                                    else { "low" };
                                                serde_json::json!({
                                                    "family": r.family,
                                                    "subfamily": r.subfamily,
                                                    "confidence": r.confidence,
                                                    "match_quality": quality
                                                })
                                            }).collect();
                                            serde_json::to_string(&suggestions).unwrap_or_default()
                                        },
                                        Err(e) => format!("{{\"error\": \"{}\"}}", e),
                                    }
                                } else {
                                    match identifier.identify_multi(&tmp_path, "RQWM", 5) {
                                        Ok(res) => serde_json::to_string(&res).unwrap_or_default(),
                                        Err(e) => format!("{{\"error\": \"{}\"}}", e),
                                    }
                                };
                                
                                let _ = std::fs::remove_file(tmp_path);
                                
                                tiny_http::Response::from_string(results)
                                    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                            }
                        }
                    },
                    _ => tiny_http::Response::from_string("{\"error\": \"Not Found\"}")
                        .with_status_code(404)
                        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
                };
                let _ = request.respond(response);
            }
        }


        None => {
            println!("{}", "🎨 FONT RESOLVER CLI".bold());
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("{}", "=".repeat(60));
            
            println!("\n{}", "🚀 QUICK COMMANDS".bold());
            println!("  {}  Resolve a font", "intellifont <font-name>".cyan());
            println!("  {}  Interactive setup", "intellifont setup".cyan());
            println!("  {}  Tiered matching (90%, 80%, internet)", "intellifont tiered <font>".cyan());
            println!("  {}  Update font database", "intellifont update".cyan());
            
            println!("\n{}", "🔧 RESOLUTION".bold());
            println!("  {}  Basic font resolution", "intellifont resolve <font> [--web]".cyan());
            println!("  {}  Tiered matching with similarity scores", "intellifont tiered <font> [--internet]".cyan());
            println!("  {}  Find similar fonts", "intellifont find-similar <font>".cyan());
            
            println!("\n{}", "⚙️  CONFIGURATION".bold());
            println!("  {}  Interactive setup", "intellifont setup".cyan());
            println!("  {}  Show current config", "intellifont config show".cyan());
            println!("  {}  Set configuration", "intellifont config set <key> <value>".cyan());
            println!("  {}  Reset to defaults", "intellifont config reset".cyan());
            
            println!("\n{}", "💾 CACHE MANAGEMENT".bold());
            println!("  {}  Show cache statistics", "fr cache stats".cyan());
            println!("  {}  Clean up cache", "fr cache cleanup".cyan());
            println!("  {}  Pin a font (never delete)", "fr cache pin <font>".cyan());
            println!("  {}  List pinned fonts", "fr cache list".cyan());
            
            println!("\n{}", "📊 INFORMATION".bold());
            println!("  {}  Show statistics", "fr stats".cyan());
            println!("  {}  Scan system fonts", "fr scan".cyan());
            println!("  {}  Check font license", "fr check-license <font>".cyan());
            println!("  {}  Update font database", "fr update".cyan());
            println!("  {}  Show version", "fr --version".cyan());
            
            println!("\n{}", "=".repeat(60));
            println!("💡 {}: Run 'fr setup' first to configure!", "Tip".bold());
            println!("📚 {}: Use 'fr --help' for detailed help", "Help".bold());
        }
    }
    
    Ok(())
}

// Helper function to load or create database
fn load_or_create_database(config: &EnhancedResolverConfig) -> Result<EnhancedFontResolver, Box<dyn std::error::Error>> {
    println!("🔄 Starting load_or_create_database...");
    let database_path = PathBuf::from("data/font_database.bin");
    
    if database_path.exists() {
        println!("📁 Database file exists at: {:?}", database_path);
        let database_data = fs::read(&database_path)?;
        println!("📊 Database file size: {} bytes", database_data.len());
        
        // Check if it's a placeholder
        if database_data.starts_with(b"MINIMAL") || database_data.starts_with(b"EMPTY") {
            println!("⚠️  Found placeholder database, creating real one...");
            let database_data = create_minimal_database()?;
            fs::write(&database_path, &database_data)?;
            
            return EnhancedFontResolver::new_with_database(config.clone(), &database_data)
                .map_err(|e| e.into());
        } else {
            // Try to load existing database
            println!("🔄 Attempting to load database...");
            match EnhancedFontResolver::new_with_database(config.clone(), &database_data) {
                Ok(resolver) => {
                    println!("✅ Successfully loaded database with {} fonts", 
                        resolver.get_database_stats()
                            .map_or(0, |s| s.font_count));
                    return Ok(resolver);
                }
                Err(e) => {
                    println!("⚠️  Failed to load database: {}", e);
                    println!("   Creating new database...");
                }
            }
        }
    } else {
        println!("❌ Database file doesn't exist");
    }
    
    // Create new database
    println!("📦 Creating new font database...");
    let database_data = create_minimal_database()?;
    
    // Save it
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    fs::write(&database_path, &database_data)?;
    
    println!("✅ Created and saved new database");
    
    // Create resolver - USE THE CORRECT FUNCTION
    font_resolver_engine::EnhancedFontResolver::new_with_database(config.clone(), &database_data)
        .map_err(|e| e.into())
}

fn print_resolution_result(result: &ResolutionResult, detailed: bool) {
    println!("✅ Resolved: {} -> {}", 
            result.original_name.bold(), 
            result.font.family.bold());
    
    if detailed {
        println!("   PostScript: {}", result.font.postscript_name);
        println!("   Path: {:?}", result.font.path);
        println!("   Weight: {}, Italic: {}, Monospaced: {}", 
                result.font.weight, result.font.italic, result.font.monospaced);
        println!("   Source: {}, Substituted: {}", result.source, result.substituted);
        println!("   Compatibility score: {:.2}", result.compatibility_score);
        
        if let Some(reason) = &result.substitution_reason {
            println!("   Substitution reason: {}", reason);
        }
    }
    
    if !result.warnings.is_empty() {
        for warning in &result.warnings {
            // Filter out license warnings for common system fonts
            if !warning.contains("Commercial font") || detailed {
                println!("⚠️  {}", warning);
            }
        }
    }
}

fn handle_memory_limit(config: &mut EnhancedResolverConfig) {
    let current_usage = match std::mem::size_of_val(config) {
        size if size < 1024 * 1024 => format!("{:.2}KB", size as f32 / 1024.0),
        size => format!("{:.2}MB", size as f32 / (1024.0 * 1024.0)),
    };
    
    println!("\n{}", "💾 MEMORY ALLOCATION".bold());
    println!("   Current configuration uses: {}", current_usage);
    println!("   Memory limit: {}MB (cache)", config.memory_limit_mb);
    println!("   Disk limit: {}MB (persistent)", config.disk_limit_mb);
    
    if config.memory_limit_mb < 2 {
        println!("{}", "⚠️  Warning: Memory limit below 2MB may not cache all fonts".yellow());
        println!("   Consider increasing: {}", "fr config set memory_limit 4".cyan());
    }
}

fn create_minimal_database() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    println!("📦 Creating compressed database...");
    
    // Try to get fonts from web database first
    let mut fonts = Vec::new();
    
    // Load web database to get popular fonts
    use font_web_db::WebFontDatabase;
    let web_db = WebFontDatabase::load_embedded();
    
    if web_db.is_loaded() {
        println!("   Loading fonts from web database...");
        
        // Get popular fonts from web database (limit to top 50 for lightweight package)
        let popular_fonts = vec![
            "Roboto", "Open Sans", "Lato", "Montserrat", "Source Sans Pro",
            "Inter", "Poppins", "Raleway", "Ubuntu", "Playfair Display",
            "Merriweather", "PT Sans", "Oswald", "Roboto Condensed", "Noto Sans",
            "Work Sans", "Fira Sans", "Nunito", "Crimson Text", "Libre Baskerville",
            "Dancing Script", "Pacifico", "Lobster", "Bebas Neue", "Barlow",
            "Rubik", "Quicksand", "Comfortaa", "Exo", "Titillium Web",
            "Muli", "Cabin", "Hind", "Arimo", "PT Serif",
            "Droid Sans", "Droid Serif", "Lora", "Inconsolata", "Fira Code",
            "JetBrains Mono", "Courier Prime", "IBM Plex Sans", "IBM Plex Serif", "IBM Plex Mono",
            "Space Mono", "Overpass", "Cantarell", "Vollkorn", "Cormorant",
        ];
        
        for font_name in popular_fonts {
            if let Some(web_font) = web_db.find_font(font_name) {
                if let Some(variant) = web_font.variants.iter()
                    .find(|v| v.weight == 400 && !v.italic)
                    .or_else(|| web_font.variants.first()) {
                    
                    let font_descriptor = web_db.to_font_descriptor(web_font, variant);
                    fonts.push(font_descriptor);
                }
            }
        }
        
        println!("   ✅ Loaded {} fonts from web database", fonts.len());
    }
    
    // Always include core system fonts as fallback
    let core_fonts = vec![
        FontDescriptor {
            family: "Arial".to_string(),
            subfamily: Some("Regular".to_string()),
            postscript_name: "arial".to_string(),
            full_name: Some("Arial Regular".to_string()),
            path: std::path::PathBuf::from("/fonts/arial.ttf"),
            format: FontFormat::Ttf,
            weight: 400,
            italic: false,
            monospaced: false,
            variable: false,
            metrics: Some(FontMetrics {
                units_per_em: 2048,
                ascender: 1854,
                descender: -434,
                x_height: 1062,
                cap_height: 1467,
                average_width: 904,
                max_advance_width: 1000,
            }),
            license: Some(LicenseInfo {
                name: "System Font".to_string(),
                url: Some("".to_string()),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            }),
        },
        FontDescriptor {
            family: "Times New Roman".to_string(),
            subfamily: Some("Regular".to_string()),
            postscript_name: "times-new-roman".to_string(),
            full_name: Some("Times New Roman Regular".to_string()),
            path: std::path::PathBuf::from("/fonts/times.ttf"),
            format: FontFormat::Ttf,
            weight: 400,
            italic: false,
            monospaced: false,
            variable: false,
            metrics: Some(FontMetrics {
                units_per_em: 2048,
                ascender: 1825,
                descender: -443,
                x_height: 916,
                cap_height: 1356,
                average_width: 818,
                max_advance_width: 1000,
            }),
            license: Some(LicenseInfo {
                name: "System Font".to_string(),
                url: Some("".to_string()),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            }),
        },
        FontDescriptor {
            family: "Courier New".to_string(),
            subfamily: Some("Regular".to_string()),
            postscript_name: "courier-new".to_string(),
            full_name: Some("Courier New Regular".to_string()),
            path: std::path::PathBuf::from("/fonts/cour.ttf"),
            format: FontFormat::Ttf,
            weight: 400,
            italic: false,
            monospaced: true,
            variable: false,
            metrics: Some(FontMetrics {
                units_per_em: 2048,
                ascender: 1705,
                descender: -615,
                x_height: 1024,
                cap_height: 1356,
                average_width: 1229,
                max_advance_width: 1229,
            }),
            license: Some(LicenseInfo {
                name: "System Font".to_string(),
                url: Some("".to_string()),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            }),
        },
        FontDescriptor {
            family: "Verdana".to_string(),
            subfamily: Some("Regular".to_string()),
            postscript_name: "verdana".to_string(),
            full_name: Some("Verdana Regular".to_string()),
            path: std::path::PathBuf::from("/fonts/verdana.ttf"),
            format: FontFormat::Ttf,
            weight: 400,
            italic: false,
            monospaced: false,
            variable: false,
            metrics: Some(FontMetrics {
                units_per_em: 2048,
                ascender: 1577,
                descender: -431,
                x_height: 1062,
                cap_height: 1467,
                average_width: 998,
                max_advance_width: 1000,
            }),
            license: Some(LicenseInfo {
                name: "System Font".to_string(),
                url: Some("".to_string()),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            }),
        },
        FontDescriptor {
            family: "Georgia".to_string(),
            subfamily: Some("Regular".to_string()),
            postscript_name: "georgia".to_string(),
            full_name: Some("Georgia Regular".to_string()),
            path: std::path::PathBuf::from("/fonts/georgia.ttf"),
            format: FontFormat::Ttf,
            weight: 400,
            italic: false,
            monospaced: false,
            variable: false,
            metrics: Some(FontMetrics {
                units_per_em: 2048,
                ascender: 1878,
                descender: -434,
                x_height: 1024,
                cap_height: 1480,
                average_width: 896,
                max_advance_width: 1000,
            }),
            license: Some(LicenseInfo {
                name: "System Font".to_string(),
                url: Some("".to_string()),
                allows_embedding: true,
                allows_modification: false,
                requires_attribution: false,
                allows_commercial_use: true,
            }),
        },
    ];
    
    // Add core system fonts to the list
    fonts.extend(core_fonts);
    
    println!("   Total fonts to compress: {}", fonts.len());
    
    // Create compressor
    let compressor = FontCompressor::new(11, true);
    
    match compressor.compress_font_database(&fonts, false) {
        Ok(compressed_data) => {
            println!("✅ Created database with {} fonts", fonts.len());
            println!("   Compressed size: {:.2}KB ({:.2}MB)", 
                    compressed_data.len() as f64 / 1024.0,
                    compressed_data.len() as f64 / (1024.0 * 1024.0));
            Ok(compressed_data)
        }
        Err(e) => {
            eprintln!("⚠️  Compression failed: {}", e);
            eprintln!("   Creating fallback JSON data...");
            
            // Fallback: create simple JSON
            let json_data = serde_json::json!({
                "fonts": fonts,
                "version": "1.0.0-minimal"
            });
            
            Ok(serde_json::to_vec(&json_data)?)
        }
    }
}

fn ensure_database_exists() -> Result<(), Box<dyn std::error::Error>> {
    let database_path = std::path::PathBuf::from("data/font_database.bin");
    
    if database_path.exists() {
        // Check if it's a valid database (not empty)
        let metadata = std::fs::metadata(&database_path)?;
        if metadata.len() > 100 { // At least 100 bytes
            return Ok(());
        }
        println!("⚠️  Database file exists but is too small ({} bytes)", metadata.len());
    }
    
    println!("📦 No valid database found. Creating minimal database...");
    
    // Create data directory if it doesn't exist
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Create minimal database
    let compressed_data = create_minimal_database()?;
    
    // Save to file
    std::fs::write(&database_path, compressed_data)?;
    
    println!("✅ Created database at: {}", database_path.display());
    println!("   Run 'fr update' to download full font database");
    
    Ok(())
}

#[derive(Parser)]
#[command(name = "intellifont", 
          about = "intelliFont Engine CLI", 
          version,
          alias = "font-resolver",
          long_about = "A smart font engine with tiered matching, internet search,\nand metric-based substitution. High-accuracy recognition for\nprofessional design workflows.",
          arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Font name to resolve (shortcut for 'resolve' command)
    #[arg(value_name = "FONT_NAME", help = "Font name to resolve (shortcut)")]
    font_name: Option<String>,
    
    /// Use web fonts in search
    #[arg(short = 'w', long = "web", help = "Enable web font search", global = true)]
    use_web_fonts: bool,
    
    /// Disable cache for this operation
    #[arg(short = 'C', long = "no-cache", help = "Disable cache for this operation", global = true)]
    no_cache: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Resolve a specific font name. This is the primary entry point for looking up local system fonts.
    #[command(alias = "r")]
    Resolve {
        /// The exact or approximate name of the font family to resolve.
        #[arg(value_name = "FONT_NAME")]
        font_name: String,
        
        /// Enable real-time fetching from web providers (Google Fonts, Fontsource) if not found locally.
        #[arg(short = 'w', long = "web")]
        use_web_fonts: bool,
        
        /// Bypass the metadata cache and perform a fresh lookup from the system/web sources.
        #[arg(short = 'C', long = "no-cache")]
        no_cache: bool,
        
        /// Output verbose diagnostic information, including paths, formats, and physical metrics.
        #[arg(short = 'd', long = "detailed")]
        detailed: bool,
    },
    
    /// Execute an advanced tiered search. This finds high-accuracy matches (90%+) or identifies the best substitutes (80%+).
    #[command(name = "tiered", alias = "t")]
    TieredResolve {
        /// The name of the font to match against the tiered similarity engine.
        #[arg(value_name = "FONT_NAME")]
        font_name: String,
        
        /// Allow the engine to search global CDNs if the best local similarity score is below the 80% threshold.
        #[arg(short = 'i', long = "internet")]
        enable_internet: bool,
    },
    
    /// Initiate the interactive 3-step configuration wizard to initialize your engine settings.
    Setup,
    
    /// Perform maintenance and monitoring on the high-performance metadata cache.
    #[command(subcommand, alias = "c")]
    Cache(CacheCommands),
    
    /// Directly modify the internal engine configuration (memory limits, providers, etc.).
    #[command(subcommand)]
    Config(ConfigCommands),
    
    /// Perform a deep recursive scan of all registered system font directories to update the internal registry.
    Scan {
        /// Display a comprehensive list of all discovered font files, grouped by format and weight.
        #[arg(long)]
        detailed: bool,
    },
    
    /// Display exhaustive engine statistics, including database health, compression ratios, and cache utilization.
    Stats,
    
    /// Identify fonts that are visually or metrically similar to a given target font.
    FindSimilar {
        /// The name of the font to use as a visual baseline for finding alternatives.
        #[arg(value_name = "FONT_NAME")]
        font_name: String,
        
        /// The maximum number of similar font candidates to return in the result set.
        #[arg(short = 'n', long = "limit", default_value = "5")]
        limit: usize,
    },
    
    /// Analyze a font's licensing metadata to determine commercial safety and provide open-source alternatives.
    CheckLicense {
        /// The name of the font whose license should be audited.
        #[arg(value_name = "FONT_NAME")]
        font_name: String,
    },
    
    /// Synchronize the local signature database with the latest global updates and regenerate optimized indexes.
    Update,
    
    /// Display full version, build architecture, and capability information.
    Version,

    /// Identify a font visually using extracted glyph signatures
    #[command(name = "identify", alias = "id")]
    Identify {
        /// Path to the font file to identify
        path: PathBuf,
        
        /// Characters to analyze (e.g. "RQWM") for higher accuracy
        #[arg(short, long, default_value = "RQWM")]
        chars: String,
        
        /// Path to glyph database (optional, defaults to data/glyph_signatures.bin)
        #[arg(long)]
        db: Option<PathBuf>,
        
        /// Show detailed signature data ("what is being passed")
        #[arg(short, long)]
        verbose: bool,

        /// Output results as JSON for programmatic use
        #[arg(long)]
        json: bool,
    },

    /// AI-powered font similarity finder
    /// 
    /// Analyzes the visual DNA of a font and suggests similar alternatives
    /// from the signature database using pattern recognition.
    #[command(name = "ai-suggest")]
    AiSuggest {
        /// Path to the font file to analyze
        path: PathBuf,
        
        /// Maximum number of suggestions to return
        #[arg(short, long, default_value = "5")]
        limit: u32,
        
        /// Path to glyph database (optional)
        #[arg(long)]
        db: Option<PathBuf>,
        
        /// Output results as JSON
        #[arg(long)]
        json: bool,
    },
    
    /// Build a glyph signature database from a directory of fonts
    #[command(name = "build-glyph-db")]
    BuildGlyphDb {
        /// Directory containing fonts to index
        source: PathBuf,
        
        /// Output path for the database file
        #[arg(short, long, default_value = "data/glyph_signatures.bin")]
        output: PathBuf,
        
        /// Compression level (1-11)
        #[arg(long, default_value = "11")]
        compression: u32,
        
        /// Recursively search for fonts
        #[arg(short, long)]
        recursive: bool,
    },

    /// Download and index popular web fonts (Google Fonts via Fontsource)
    #[command(name = "build-web-db")]
    BuildWebDb {
        /// Limit number of fonts to download (default: 50)
        #[arg(short, long, default_value = "50")]
        limit: usize,
        
        /// Output path
        #[arg(short, long, default_value = "data/web_glyph_signatures.bin")]
        output: PathBuf,
    },

    
    /// Show statistics about a glyph database
    GlyphDbStats {
        /// Path to the database file
        #[arg(default_value = "data/glyph_signatures.bin")]
        path: PathBuf,
    },

    /// Run as a high-performance HTTP microservice
    #[command(name = "serve")]
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Host to bind to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,

        /// Path to glyph database
        #[arg(long, default_value = "data/glyph_signatures.bin")]
        db: PathBuf,
    },
}

#[derive(Subcommand)]
enum CacheCommands {
    /// Show detailed cache utilization, including hit rates and memory/disk footprints.
    Stats,
    
    /// Optimized cache pruning. Removes stale or low-priority entries to reclaim system resources.
    Cleanup {
        /// Maximize resource recovery by removing all entries that haven't reached the auto-pin threshold.
        #[arg(long)]
        aggressive: bool,
        
        /// Preview the cleanup operation without deleting any physical files or entries.
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Lock a specific font in the cache so it is never removed during automatic cleanup operations.
    Pin {
        /// The name of the font family to permanently cache.
        font_name: String,
    },
    
    /// Remove the permanent lock from a font, allowing it to be managed by the standard cache eviction logic.
    Unpin {
        /// The name of the font family to release from the pin.
        font_name: String,
    },
    
    /// Display a list of all manually and automatically pinned fonts.
    List,
    
    /// Analyze cache usage and suggest specific entries for manual removal to improve performance.
    Suggest,
}

#[derive(Subcommand, Clone)]
enum ConfigCommands {
    /// Show current configuration
    Show,
    
    /// Set configuration value
    Set {
        /// Configuration key (memory_limit, web_fonts, etc.)
        key: String,
        
        /// Value to set
        value: String,
    },
    
    /// Reset to defaults
    Reset,
    
    /// Export configuration to file
    Export {
        /// Path to export configuration to
        path: String,
    },
    
    /// Import configuration from file
    Import {
        /// Path to import configuration from
        path: String,
    },
}

#[derive(ValueEnum, Clone)]
enum ConfigKey {
    MemoryLimit,
    DiskLimit,
    WebFonts,
    LicenseWarnings,
    AutoPin,
}