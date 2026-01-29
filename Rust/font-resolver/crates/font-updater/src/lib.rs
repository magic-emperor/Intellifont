use std::collections::HashMap;
use std::path::{Path, PathBuf};
use font_core::FontResult;
use font_compressor::{CompressedFontDatabase, FontCompressor};
use font_acquisition::FontAcquisitionManager;
use serde::{Deserialize, Serialize};
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Sha256, Digest};
use futures_util::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub font_count: usize,
    pub total_size_bytes: usize,
    pub compressed_size_bytes: usize,
    pub created_at: String,
    pub checksum: String,
    pub incremental_from: Option<String>,
    pub changes: UpdateChanges,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChanges {
    pub added_fonts: Vec<String>,
    pub removed_fonts: Vec<String>,
    pub updated_fonts: Vec<String>,
    pub security_fixes: Vec<String>,
}

pub struct FontUpdater {
    base_path: PathBuf,
    acquisition_manager: FontAcquisitionManager,
    compression_quality: u32,
}

impl FontUpdater {
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            acquisition_manager: FontAcquisitionManager::new(),
            compression_quality: 11,
        }
    }
    
    pub async fn check_for_updates(&self, _current_version: &str) -> FontResult<Option<UpdateManifest>> {
        // Simulating update check - in real implementation, fetch from server
        // For now, return None to indicate no updates
        Ok(None)
    }
    
    pub async fn download_incremental_update(
        &self,
        manifest: &UpdateManifest,
        progress_callback: Option<Box<dyn Fn(u64, u64) + Send>>,
    ) -> FontResult<PathBuf> {
        println!("📥 Downloading update v{}...", manifest.version);
        
        // Create mock URL for demonstration
        let update_url = format!(
            "https://updates.font-resolver.com/updates/{}.bin",
            manifest.version
        );
        
        let client = reqwest::Client::new();
        let response = client.get(&update_url).send().await
            .map_err(|e| font_core::FontError::Parse(format!("Download failed: {}", e)))?;
        
        let total_size = response.content_length().unwrap_or(manifest.compressed_size_bytes as u64);
        
        // Setup progress bar
        let pb = if progress_callback.is_none() {
            let pb = ProgressBar::new(total_size);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"));
            Some(pb)
        } else {
            None
        };
        
        let mut downloaded = Vec::with_capacity(manifest.compressed_size_bytes);
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes: u64 = 0;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| font_core::FontError::Parse(format!("Stream error: {}", e)))?;
            
            downloaded.extend_from_slice(&chunk);
            downloaded_bytes += chunk.len() as u64;
            
            // Update progress
            if let Some(ref callback) = progress_callback {
                callback(downloaded_bytes, total_size);
            } else if let Some(ref pb) = pb {
                pb.set_position(downloaded_bytes);
            }
        }
        
        if let Some(pb) = pb {
            pb.finish_with_message("✅ Download complete");
        }
        
        // Verify checksum
        let checksum = self.calculate_checksum(&downloaded);
        if checksum != manifest.checksum {
            return Err(font_core::FontError::Parse(
                format!("Checksum verification failed: expected {}, got {}", manifest.checksum, checksum)
            ));
        }
        
        // Save update file
        let update_path = self.base_path.join(format!("update_{}.bin", manifest.version));
        tokio::fs::write(&update_path, &downloaded).await
            .map_err(|e| font_core::FontError::Io(e))?;
        
        println!("💾 Saved update to: {:?}", update_path);
        
        Ok(update_path)
    }
    
    pub async fn apply_incremental_update(
        &self,
        update_path: &Path,
        current_database: &CompressedFontDatabase,
    ) -> FontResult<CompressedFontDatabase> {
        println!("🔄 Applying update...");
        
        // Load update
        let update_data = tokio::fs::read(update_path).await
            .map_err(|e| font_core::FontError::Io(e))?;
        
        let decompressor = FontCompressor::new(11, true);
        let update_database = decompressor.decompress_font_database(&update_data)
            .map_err(|e| font_core::FontError::Parse(e.to_string()))?;
        
        println!("📊 Update contains {} fonts", update_database.metadata.font_count);
        
        // Merge databases
        let merged = self.merge_databases(current_database, &update_database).await?;
        
        // Recompress
        let compressor = FontCompressor::new(self.compression_quality, true);
        let fonts_for_compression: Vec<_> = merged.fonts.iter()
            .map(|f| self.compressed_to_font(f))
            .collect();
        
        let compressed = compressor.compress_font_database(
            &fonts_for_compression,
            true,
        ).map_err(|e| font_core::FontError::Parse(e.to_string()))?;
        
        // Update metadata with correct compressed size
        let mut merged_with_size = merged;
        merged_with_size.metadata.compressed_size_bytes = compressed.len();
        
        // Save new database
        let new_path = self.base_path.join("fonts_latest.bin");
        tokio::fs::write(&new_path, &compressed).await
            .map_err(|e| font_core::FontError::Io(e))?;
        
        println!("✅ Update applied! New database: {} fonts, {:.2}MB", 
            merged_with_size.metadata.font_count,
            merged_with_size.metadata.compressed_size_bytes as f64 / (1024.0 * 1024.0)
        );
        
        Ok(merged_with_size)
    }
    
    pub async fn update_from_internet(
        &self,
        limit: usize,
        categories: Option<Vec<font_compressor::FontCategory>>,
    ) -> FontResult<CompressedFontDatabase> {
        println!("🌐 Starting internet font update...");
        
        // Search for popular fonts - FIX: Store length before moving
        let popular_queries = vec![
            "sans", "serif", "mono", "display", "handwriting",
            "roboto", "open sans", "lato", "montserrat", "source sans",
        ];
        
        let queries_len = popular_queries.len();
        let mut all_fonts = Vec::new();
        
        for query in &popular_queries { // FIX: Use reference to avoid moving
            println!("🔍 Searching for: {}...", query);
            
            // FIX: Use queries_len instead of popular_queries.len()
            let limit_per_query = if queries_len > 0 { limit / queries_len } else { limit };
            
            match self.acquisition_manager.parallel_search(query, limit_per_query).await {
                Ok(fonts) => {
                    println!("   Found {} fonts", fonts.len());
                    all_fonts.extend(fonts);
                }
                Err(e) => eprintln!("⚠️  Failed to search for {}: {}", query, e),
            }
            
            // Limit total
            if all_fonts.len() >= limit {
                break;
            }
        }
        
        // Filter by category if specified
        if let Some(categories) = &categories {
            all_fonts.retain(|font| categories.contains(&font.category));
        }
        
        // Deduplicate
        let mut seen = std::collections::HashSet::new();
        all_fonts.retain(|font| seen.insert(font.family.clone()));
        
        println!("📊 Total unique fonts found: {}", all_fonts.len());
        
        // Build database
        let compressor = FontCompressor::new(self.compression_quality, true);
        let fonts_for_compression: Vec<_> = all_fonts.iter()
            .map(|f| self.compressed_to_font(f))
            .collect();
        
        let compressed = compressor.compress_font_database(
            &fonts_for_compression,
            true,
        ).map_err(|e| font_core::FontError::Parse(e.to_string()))?;
        
        // Create database with correct metadata
        let database = CompressedFontDatabase {
            metadata: font_compressor::FontDatabaseMetadata {
                version: env!("CARGO_PKG_VERSION").to_string(),
                font_count: all_fonts.len(),
                compressed_size_bytes: compressed.len(),
                original_size_bytes: all_fonts.iter().map(|f| f.file_size_kb as usize * 1024).sum(),
                created_at: chrono::Utc::now().to_rfc3339(),
                categories: all_fonts.iter()
                    .fold(HashMap::new(), |mut acc, font| {
                        *acc.entry(font.category.clone()).or_insert(0) += 1;
                        acc
                    }),
                include_full_data: true,
            },
            fonts: all_fonts,
            similarity_matrix: None,
        };
        
        println!("✅ Updated database with {} fonts ({:.2}MB compressed)", 
            database.metadata.font_count,
            database.metadata.compressed_size_bytes as f64 / (1024.0 * 1024.0)
        );
        
        Ok(database)
    }
    
    pub async fn user_initiated_update(&self) -> FontResult<()> {
        println!("🔄 User-Initiated Font Update");
        println!("=============================");
        
        let options = vec![
            "1. Update from official repository (recommended)",
            "2. Search and download specific fonts",
            "3. Update similarity data only",
            "4. Clean and rebuild database",
        ];
        
        for option in options {
            println!("{}", option);
        }
        
        // In real implementation, get user input
        let choice = 1; // Example
        
        match choice {
            1 => {
                println!("Checking for updates...");
                if let Some(manifest) = self.check_for_updates("0.1.0").await? {
                    println!("Found update v{} with {} fonts", 
                        manifest.version, manifest.font_count);
                    
                    let update_path = self.download_incremental_update(&manifest, None).await?;
                    println!("✅ Update downloaded successfully");
                    
                    // Load current database and apply update
                    let current_db = self.load_current_database().await?;
                    self.apply_incremental_update(&update_path, &current_db).await?;
                } else {
                    println!("✅ Already up to date");
                }
            }
            2 => {
                println!("Enter font names to search (comma separated):");
                // Get user input and search
            }
            _ => println!("Option not implemented yet"),
        }
        
        Ok(())
    }
    
    fn calculate_checksum(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
    
    async fn merge_databases(
        &self,
        current: &CompressedFontDatabase,
        update: &CompressedFontDatabase,
    ) -> FontResult<CompressedFontDatabase> {
        let mut merged_fonts = current.fonts.clone();
        let mut font_map: HashMap<String, usize> = current.fonts
            .iter()
            .enumerate()
            .map(|(i, f)| (f.family.clone(), i))
            .collect();
        
        let mut added = 0;
        let mut updated = 0;
        
        for update_font in &update.fonts {
            if let Some(&idx) = font_map.get(&update_font.family) {
                // Update existing font
                merged_fonts[idx] = update_font.clone();
                updated += 1;
            } else {
                // Add new font
                merged_fonts.push(update_font.clone());
                font_map.insert(update_font.family.clone(), merged_fonts.len() - 1);
                added += 1;
            }
        }
        
        println!("📊 Merge stats: Added {}, Updated {}", added, updated);
        
        Ok(CompressedFontDatabase {
            metadata: font_compressor::FontDatabaseMetadata {
                version: update.metadata.version.clone(),
                font_count: merged_fonts.len(),
                compressed_size_bytes: 0, // Will be set after compression
                original_size_bytes: merged_fonts.iter().map(|f| f.file_size_kb as usize * 1024).sum(),
                created_at: chrono::Utc::now().to_rfc3339(),
                categories: merged_fonts.iter()
                    .fold(HashMap::new(), |mut acc, font| {
                        *acc.entry(font.category.clone()).or_insert(0) += 1;
                        acc
                    }),
                include_full_data: current.metadata.include_full_data || update.metadata.include_full_data,
            },
            fonts: merged_fonts,
            similarity_matrix: None, // Will be regenerated
        })
    }
    
    async fn load_current_database(&self) -> FontResult<CompressedFontDatabase> {
        // Load from default path
        let default_path = self.base_path.join("font_database.bin");
        if default_path.exists() {
            let data = tokio::fs::read(&default_path).await
                .map_err(|e| font_core::FontError::Io(e))?;
            
            let decompressor = FontCompressor::new(11, true);
            decompressor.decompress_font_database(&data)
                .map_err(|e| font_core::FontError::Parse(e.to_string()))
        } else {
            // Return empty database
            Ok(CompressedFontDatabase {
                metadata: font_compressor::FontDatabaseMetadata {
                    version: "0.1.0".to_string(),
                    font_count: 0,
                    compressed_size_bytes: 0,
                    original_size_bytes: 0,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    categories: HashMap::new(),
                    include_full_data: true,
                },
                fonts: Vec::new(),
                similarity_matrix: None,
            })
        }
    }
    
    fn compressed_to_font(&self, compressed: &font_compressor::CompressedFontData) -> font_core::FontDescriptor {
        font_core::FontDescriptor {
            family: compressed.family.clone(),
            subfamily: None,
            postscript_name: compressed.postscript_name.clone(),
            full_name: Some(compressed.family.clone()),
            path: std::path::PathBuf::from("/compressed"),
            format: font_core::FontFormat::Ttf,
            weight: compressed.weight,
            italic: compressed.italic,
            monospaced: compressed.monospaced,
            variable: false,
            metrics: compressed.metrics.as_ref().map(|m| font_core::FontMetrics {
                units_per_em: m.units_per_em,
                ascender: m.ascender,
                descender: m.descender,
                x_height: m.x_height,
                cap_height: m.cap_height,
                average_width: m.average_width,
                max_advance_width: 1000,
            }),
            license: Some(font_core::LicenseInfo {
                name: compressed.license.name.clone(),
                url: Some(compressed.license.url.clone()),
                allows_embedding: compressed.license.allows_embedding,
                allows_modification: compressed.license.allows_modification,
                requires_attribution: compressed.license.requires_attribution,
                allows_commercial_use: compressed.license.allows_commercial_use,
            }),
        }
    }
}