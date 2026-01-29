use std::path::PathBuf;
use dialoguer::Confirm;
use directories::ProjectDirs;
use font_core::{EnhancedResolverConfig, LicenseWarningLevel, SetupConfig, FontSourcePriority};

pub fn interactive_setup() -> SetupConfig {
    println!("\n{}", "=".repeat(50));
    println!("🎨 FONT RESOLVER - QUICK SETUP");
    println!("{}", "=".repeat(50));
    
    // Question 1: Memory limit (fixed at 2MB, just inform)
    println!("\n📊 1. Memory limit: 2MB (fixed)");
    println!("   ↪ Enough for all system fonts with room for growth");
    println!("   ↪ You can adjust this later with: fr config set memory.limit_mb <size>");
    
    // Question 2: Web fonts
    println!("\n🌐 2. Web fonts:");
    println!("   ↪ Adds 1500+ fonts from Google Fonts database");
    println!("   ↪ Increases package size by 400KB");
    
    let enable_web_fonts = Confirm::new()
        .with_prompt("   Enable web fonts?")
        .default(true)
        .interact()
        .unwrap_or(true);
    
    // Question 3: License warnings
    println!("\n⚖️  3. License warnings:");
    println!("   ↪ Shows warnings for commercial font usage");
    println!("   ↪ Suggests free alternatives when possible");
    
    let enable_license_warnings = Confirm::new()
        .with_prompt("   Enable license warnings?")
        .default(true)
        .interact()
        .unwrap_or(true);
    
    println!("\n{}", "=".repeat(50));
    
    let confirm = Confirm::new()
        .with_prompt("\nApply these settings?")
        .default(true)
        .interact()
        .unwrap_or(true);
    
    if confirm {
        SetupConfig {
            memory_limit_mb: 2,
            enable_web_fonts,
            enable_license_warnings,
            auto_pin_fonts: true,
        }
    } else {
        println!("\nSetup cancelled. Using minimal defaults.");
        println!("You can run setup later with: fr setup");
        SetupConfig::default()
    }
}

pub fn apply_setup(config: &SetupConfig) -> EnhancedResolverConfig {
    EnhancedResolverConfig {
        base: font_core::ResolverConfig::default(),
        cache_enabled: true,
        memory_limit_mb: config.memory_limit_mb,
        disk_limit_mb: 10,
        auto_pin_threshold: if config.auto_pin_fonts { 5 } else { 0 },
        cache_cleanup_mode: font_core::CacheCleanupMode::Manual,
        system_fonts_enabled: true,
        web_fonts_enabled: config.enable_web_fonts,
        custom_fonts_enabled: false,
        font_source_priority: if config.enable_web_fonts {
            FontSourcePriority::SystemThenWeb
        } else {
            FontSourcePriority::SystemOnly
        },
        license_warnings: if config.enable_license_warnings {
            LicenseWarningLevel::All
        } else {
            LicenseWarningLevel::Off
        },
        watermark_commercial: false,
        auto_setup_completed: true,
        telemetry_enabled: false,
        dynamic_learning_enabled: true,
        project_asset_dirs: Vec::new(),
    }
}

pub fn load_config() -> Result<EnhancedResolverConfig, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    
    if config_path.exists() {
        let config_str = std::fs::read_to_string(&config_path)?;
        let config: EnhancedResolverConfig = toml::from_str(&config_str)?;
        Ok(config)
    } else {
        // No config file exists, use defaults
        Ok(EnhancedResolverConfig::default())
    }
}

pub fn save_config(config: &EnhancedResolverConfig) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    
    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let config_str = toml::to_string_pretty(config)?;
    std::fs::write(config_path, config_str)?;
    
    Ok(())
}

pub fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let project_dirs = ProjectDirs::from("com", "font-resolver", "config")
        .ok_or("Could not determine config directory")?;
    
    Ok(project_dirs.config_dir().join("config.toml"))
}

pub fn show_current_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    
    println!("{}", "=".repeat(50));
    println!("📋 CURRENT CONFIGURATION");
    println!("{}", "=".repeat(50));
    
    println!("\n💾 Cache Settings:");
    println!("   Enabled: {}", config.cache_enabled);
    println!("   Memory limit: {}MB", config.memory_limit_mb);
    println!("   Disk limit: {}MB", config.disk_limit_mb);
    println!("   Auto-pin threshold: {} uses", config.auto_pin_threshold);
    println!("   Cleanup mode: {:?}", config.cache_cleanup_mode);
    
    println!("\n🔤 Font Sources:");
    println!("   System fonts: {}", config.system_fonts_enabled);
    println!("   Web fonts: {}", config.web_fonts_enabled);
    println!("   Custom fonts: {}", config.custom_fonts_enabled);
    println!("   Priority: {:?}", config.font_source_priority);
    
    println!("\n⚖️  License Settings:");
    println!("   Warnings: {:?}", config.license_warnings);
    println!("   Watermark commercial: {}", config.watermark_commercial);
    
    println!("\n🚀 Performance:");
    println!("   Setup completed: {}", config.auto_setup_completed);
    println!("   Telemetry: {}", config.telemetry_enabled);
    
    println!("\n{}", "=".repeat(50));
    
    Ok(())
}