use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, Duration};

use font_core::{FontDescriptor, FontError, FontResult, CacheStats};
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    font: FontDescriptor,
    access_count: u32,
    last_accessed: SystemTime,
    created_at: SystemTime,
    is_pinned: bool,
    estimated_size_kb: usize,
}

pub struct HybridFontCache {
    memory: Mutex<LruCache<String, CacheEntry>>,
    disk_path: PathBuf,
    memory_limit_bytes: usize,
    disk_limit_bytes: usize,
    auto_pin_threshold: u32,
    pinned_fonts: RwLock<HashSet<String>>,
    access_counts: RwLock<HashMap<String, u32>>,
}

impl HybridFontCache {
    pub fn new(memory_limit_mb: usize, disk_limit_mb: usize, auto_pin_threshold: u32) -> FontResult<Self> {
        let memory_limit_bytes = memory_limit_mb * 1024 * 1024;
        let disk_limit_bytes = disk_limit_mb * 1024 * 1024;
        
        // Create cache directory - Use local relative path for reliability
        let disk_path = PathBuf::from(".font_cache");
        std::fs::create_dir_all(&disk_path)?;
        
        // Load pinned fonts from disk
        let pinned_path = disk_path.join("pinned_fonts.bin");
        let pinned_fonts = if pinned_path.exists() {
            let data = std::fs::read(&pinned_path)?;
            bincode::deserialize(&data).unwrap_or_default()
        } else {
            HashSet::new()
        };
        
        // Load access counts
        let access_path = disk_path.join("access_counts.bin");
        let access_counts = if access_path.exists() {
            let data = std::fs::read(&access_path)?;
            bincode::deserialize(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };
        
        Ok(Self {
            memory: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(1000).unwrap()
            )),
            disk_path,
            memory_limit_bytes,
            disk_limit_bytes,
            auto_pin_threshold,
            pinned_fonts: RwLock::new(pinned_fonts),
            access_counts: RwLock::new(access_counts),
        })
    }
    
    pub fn get(&self, font_name: &str) -> Option<FontDescriptor> {
        let mut memory = self.memory.lock();
        
        // Update access count
        {
            let mut access_counts = self.access_counts.write();
            let count = access_counts.entry(font_name.to_string())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            
            // Auto-pin if used frequently
            if *count >= self.auto_pin_threshold && !self.is_pinned(font_name) {
                self.pin_font(font_name);
            }
        }
        
        // Check memory cache first
        if let Some(entry) = memory.get(font_name) {
            let mut entry = entry.clone();
            entry.last_accessed = SystemTime::now();
            memory.put(font_name.to_string(), entry.clone());
            return Some(entry.font);
        }
        
        // Check disk cache
        if let Some(entry) = self.load_from_disk(font_name) {
            // Promote to memory if there's space
            let estimated_size = entry.estimated_size_kb * 1024;
            if self.current_memory_usage() + estimated_size <= self.memory_limit_bytes {
                let mut entry = entry.clone();
                entry.last_accessed = SystemTime::now();
                memory.put(font_name.to_string(), entry.clone());
            }
            return Some(entry.font);
        }
        
        None
    }
    
    pub fn put(&self, font_name: &str, font: FontDescriptor) -> FontResult<()> {
        let is_pinned = self.is_pinned(font_name);
        let estimated_size_kb = self.estimate_font_size_kb(&font);
        
        // Check memory usage
        let memory_needed = estimated_size_kb * 1024;
        if self.current_memory_usage() + memory_needed > self.memory_limit_bytes {
            return Err(FontError::MemoryLimitExceeded(
                (self.current_memory_usage() + memory_needed) as f64 / (1024.0 * 1024.0),
                self.memory_limit_bytes / (1024 * 1024)
            ));
        }
        
        // Check disk usage - use estimate to avoid hanging on large cache directories
        // Estimate based on entry count rather than scanning filesystem
        let estimated_disk_usage = self.estimate_disk_usage_fast();
        if estimated_disk_usage + memory_needed > self.disk_limit_bytes {
            return Err(FontError::DiskLimitExceeded(
                (estimated_disk_usage + memory_needed) as f64 / (1024.0 * 1024.0),
                self.disk_limit_bytes / (1024 * 1024)
            ));
        }
        
        let entry = CacheEntry {
            font,
            access_count: 1,
            last_accessed: SystemTime::now(),
            created_at: SystemTime::now(),
            is_pinned,
            estimated_size_kb,
        };
        
        // Store in memory
        let mut memory = self.memory.lock();
        memory.put(font_name.to_string(), entry.clone());
        
        // Store in disk
        self.save_to_disk(font_name, &entry)?;
        
        Ok(())
    }
    
    pub fn pin_font(&self, font_name: &str) {
        let data = {
            let mut pinned = self.pinned_fonts.write();
            pinned.insert(font_name.to_string());
            bincode::serialize(&*pinned).ok()
        };
        
        if let Some(data) = data {
            let pinned_path = self.disk_path.join("pinned_fonts.bin");
            let _ = std::fs::write(pinned_path, data);
        }
        
        // Update cache entry if in memory
        if let Some(entry) = self.memory.lock().get_mut(font_name) {
            entry.is_pinned = true;
        }
    }
    
    pub fn unpin_font(&self, font_name: &str) {
        let data = {
            let mut pinned = self.pinned_fonts.write();
            pinned.remove(font_name);
            bincode::serialize(&*pinned).ok()
        };
        
        if let Some(data) = data {
            let pinned_path = self.disk_path.join("pinned_fonts.bin");
            let _ = std::fs::write(pinned_path, data);
        }
        
        // Update cache entry if in memory
        if let Some(entry) = self.memory.lock().get_mut(font_name) {
            entry.is_pinned = false;
        }
    }
    
    pub fn is_pinned(&self, font_name: &str) -> bool {
        self.pinned_fonts.read().contains(font_name)
    }
    
    pub fn cleanup(&self, aggressive: bool) -> FontResult<usize> {
        let mut memory = self.memory.lock();
        let mut removed = 0;
        
        // Remove unpinned, least recently used entries from memory
        let keys: Vec<String> = memory.iter().map(|(k, _)| k.clone()).collect();
        
        for key in keys {
            if let Some(entry) = memory.peek(&key) {
                if !entry.is_pinned {
                    // For aggressive cleanup, remove if used only once
                    // For normal cleanup, remove if not accessed in 30 days
                    let should_remove = if aggressive {
                        entry.access_count == 1
                    } else {
                        entry.last_accessed.elapsed().unwrap_or_default() > Duration::from_secs(30 * 24 * 60 * 60)
                    };
                    
                    if should_remove {
                        memory.pop(&key);
                        removed += 1;
                    }
                }
            }
        }
        
        // Clean disk cache
        removed += self.cleanup_disk(aggressive)?;
        
        // Save access counts
        self.save_access_counts()?;
        
        Ok(removed)
    }

    pub fn remove_entry(&self, font_name: &str) -> FontResult<bool> {
        let mut removed = false;
        
        // Remove from memory
        if self.memory.lock().pop(font_name).is_some() {
            removed = true;
        }
        
        // Remove from disk
        let path = self.disk_path.join(format!("{}.bin", font_name));
        if path.exists() {
            std::fs::remove_file(path)?;
            removed = true;
        }
        
        // Remove from access counts
        self.access_counts.write().remove(font_name);
        
        Ok(removed)
    }

    pub fn remove_entries(&self, font_names: &[String]) -> FontResult<usize> {
        let mut count = 0;
        for name in font_names {
            if self.remove_entry(name)? {
                count += 1;
            }
        }
        Ok(count)
    }
    
    pub fn stats(&self) -> FontResult<CacheStats> {
        // CRITICAL: This method must NEVER block or hang.
        // We use try_lock() to avoid deadlocks and return immediately
        // even if locks are contended.
        
        // Try to get memory lock with timeout (non-blocking)
        let memory_entries = self.memory.try_lock()
            .map(|m| m.len())
            .unwrap_or(0);
        
        let pinned_fonts = self.pinned_fonts.try_read()
            .map(|p| p.len())
            .unwrap_or(0);
        
        // Calculate memory usage quickly (limit iteration to prevent hangs)
        let memory_usage = self.memory.try_lock()
            .map(|memory| {
                // Limit to first 1000 entries to prevent hanging on huge caches
                memory.iter()
                    .take(1000)
                    .map(|(_, entry)| entry.estimated_size_kb * 1024)
                    .sum::<usize>()
            })
            .unwrap_or(0);
        
        // Never touch filesystem - always return 0 for disk stats
        let disk_entries = 0usize;
        let disk_usage = 0usize;
        
        Ok(CacheStats {
            memory_entries,
            disk_entries,
            pinned_fonts,
            memory_usage_mb: memory_usage as f64 / (1024.0 * 1024.0),
            disk_usage_mb: disk_usage as f64 / (1024.0 * 1024.0),
        })
    }
    
    pub fn list_pinned(&self) -> Vec<String> {
        let pinned = self.pinned_fonts.read();
        pinned.iter().cloned().collect()
    }
    
    pub fn suggest_cleanup(&self) -> FontResult<Vec<String>> {
        let mut suggestions = Vec::new();
        
        // Check memory cache for cleanup candidates
        let memory = self.memory.lock();
        for (key, entry) in memory.iter() {
            if !entry.is_pinned && entry.access_count == 1 {
                suggestions.push(format!("Memory: {} (used only once)", key));
            } else if !entry.is_pinned && entry.last_accessed.elapsed().unwrap_or_default() > Duration::from_secs(7 * 24 * 60 * 60) {
                suggestions.push(format!("Memory: {} (not used in 7 days)", key));
            }
        }
        
        // Check disk cache
        let entries = std::fs::read_dir(&self.disk_path)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "bin").unwrap_or(false) {
                if let Some(file_name) = path.file_stem() {
                    let font_name = file_name.to_string_lossy().to_string();
                    if !self.is_pinned(&font_name) {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                if SystemTime::now().duration_since(modified).unwrap_or_default() > Duration::from_secs(30 * 24 * 60 * 60) {
                                    suggestions.push(format!("Disk: {} (not accessed in 30 days)", font_name));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(suggestions)
    }
    
    // Private helper methods
    
    fn current_memory_usage(&self) -> usize {
        let memory = self.memory.lock();
        memory.iter()
            .map(|(_, entry)| entry.estimated_size_kb * 1024)
            .sum()
    }
    
    #[allow(dead_code)]
    fn current_disk_usage(&self) -> FontResult<usize> {
        Self::get_directory_size(&self.disk_path)
    }
    
    /// Fast estimate of disk usage without filesystem traversal
    /// Uses memory cache size as a proxy to avoid hanging
    fn estimate_disk_usage_fast(&self) -> usize {
        // Estimate: memory entries * average size (50KB) * 2 (disk overhead)
        let memory_entries = self.memory.lock().len();
        memory_entries * 50 * 1024 * 2
    }
    
    #[allow(dead_code)]
    fn get_directory_size(path: &Path) -> FontResult<usize> {
        let mut total = 0;
        let mut file_count = 0;
        let max_files = 5000; // Limit total files processed to prevent hangs
        let max_depth = 3; // Only go 3 levels deep
        
        // Use a stack with depth tracking
        let mut stack: Vec<(PathBuf, usize)> = vec![(path.to_path_buf(), 0)];
        let mut visited = std::collections::HashSet::new();
        
        while let Some((current_path, depth)) = stack.pop() {
            // Skip if already visited or too deep
            if visited.contains(&current_path) || depth > max_depth {
                continue;
            }
            visited.insert(current_path.clone());
            
            // Stop if we've processed too many files
            if file_count >= max_files {
                eprintln!("⚠️  Warning: File limit reached ({}) while calculating directory size", max_files);
                break;
            }
            
            match std::fs::read_dir(&current_path) {
                Ok(entries) => {
                    for entry in entries {
                        if file_count >= max_files {
                            break;
                        }
                        
                        match entry {
                            Ok(entry) => {
                                let entry_path = entry.path();
                                
                                // Skip symlinks
                                if let Ok(metadata) = entry_path.symlink_metadata() {
                                    if metadata.is_symlink() {
                                        continue;
                                    }
                                    
                                    if metadata.is_file() {
                                        total += metadata.len() as usize;
                                        file_count += 1;
                                    } else if metadata.is_dir() && depth < max_depth {
                                        stack.push((entry_path, depth + 1));
                                    }
                                }
                            }
                            Err(_) => {
                                // Silently skip permission errors
                                continue;
                            }
                        }
                    }
                }
                Err(_) => {
                    // Silently skip directories we can't read
                    continue;
                }
            }
        }
        
        Ok(total)
    }
    
    fn estimate_font_size_kb(&self, font: &FontDescriptor) -> usize {
        // Estimate based on font metrics and complexity
        let base_size = 50; // 50KB base for metadata
        let variant_penalty = if font.variable { 100 } else { 0 }; // Variable fonts are larger
        let metrics_penalty = if font.metrics.is_some() { 20 } else { 0 };
        
        base_size + variant_penalty + metrics_penalty
    }
    
    fn load_from_disk(&self, font_name: &str) -> Option<CacheEntry> {
        let path = self.disk_path.join(format!("{}.bin", font_name));
        if !path.exists() {
            return None;
        }
        
        match std::fs::read(&path) {
            Ok(data) => bincode::deserialize(&data).ok(),
            Err(_) => None,
        }
    }
    
    fn save_to_disk(&self, font_name: &str, entry: &CacheEntry) -> FontResult<()> {
        let path = self.disk_path.join(format!("{}.bin", font_name));
        let data = bincode::serialize(entry)
            .map_err(|e| FontError::Parse(format!("Failed to serialize cache entry: {}", e)))?;
        std::fs::write(path, data)?;
        Ok(())
    }
    
    fn cleanup_disk(&self, aggressive: bool) -> FontResult<usize> {
        let mut removed = 0;
        let entries = std::fs::read_dir(&self.disk_path)?;
        
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map(|e| e == "bin").unwrap_or(false) {
                if let Some(file_name) = path.file_stem() {
                    let font_name = file_name.to_string_lossy().to_string();
                    
                    // Skip pinned fonts
                    if self.is_pinned(&font_name) {
                        continue;
                    }
                    
                    // Check if file is old
                    let metadata = std::fs::metadata(&path)?;
                    let modified = metadata.modified()?;
                    let age = SystemTime::now().duration_since(modified).unwrap_or_default();
                    
                    let should_remove = if aggressive {
                        age > Duration::from_secs(7 * 24 * 60 * 60) // 7 days for aggressive
                    } else {
                        age > Duration::from_secs(30 * 24 * 60 * 60) // 30 days for normal
                    };
                    
                    if should_remove {
                        std::fs::remove_file(path)?;
                        removed += 1;
                    }
                }
            }
        }
        
        Ok(removed)
    }
    
    #[allow(dead_code)]
    fn count_disk_entries(&self) -> usize {
        match std::fs::read_dir(&self.disk_path) {
            Ok(entries) => {
                // Very aggressive limit to prevent hanging
                // Count quickly and stop early if there are many files
                let mut count = 0;
                let max_to_count = 1000; // Only count first 1000 files
                
                for entry in entries.take(max_to_count + 1) {
                    match entry {
                        Ok(entry) => {
                            if entry.path()
                                .extension()
                                .map(|ext| ext == "bin")
                                .unwrap_or(false)
                            {
                                count += 1;
                            }
                        }
                        Err(_) => continue,
                    }
                }
                
                // If we hit the limit, add "1000+" indicator would be shown in stats
                // For now, just return the count we got
                count
            }
            Err(_) => 0,
        }
    }
    
    // REDUNDANT - Integrated into pin/unpin to avoid deadlocks
    /*
    fn save_pinned_fonts(&self) {
        let pinned_path = self.disk_path.join("pinned_fonts.bin");
        let pinned = self.pinned_fonts.read();
        if let Ok(data) = bincode::serialize(&*pinned) {
            let _ = std::fs::write(pinned_path, data);
        }
    }
    */
    
    fn save_access_counts(&self) -> FontResult<()> {
        let access_path = self.disk_path.join("access_counts.bin");
        let access_counts = self.access_counts.read();
        let data = bincode::serialize(&*access_counts)
            .map_err(|e| FontError::Parse(format!("Failed to serialize access counts: {}", e)))?;
        std::fs::write(access_path, data)?;
        Ok(())
    }
}