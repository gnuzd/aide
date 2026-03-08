use sysinfo::System;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemSpecs {
    pub os_name: String,
    pub os_version: String,
    pub total_memory_gb: u64,
    pub available_memory_gb: u64,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
}

impl SystemSpecs {
    /// Collect current system hardware information
    pub fn audit() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        // In sysinfo 0.30+, name() and os_version() are associated functions
        let os_name = System::name().unwrap_or_else(|| "Unknown OS".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown version".to_string());
        
        // Convert bytes to GB
        let total_memory_gb = sys.total_memory() / (1024 * 1024 * 1024);
        let available_memory_gb = sys.available_memory() / (1024 * 1024 * 1024);

        let cpus = sys.cpus();
        let cpu_brand = if !cpus.is_empty() {
            cpus[0].brand().to_string()
        } else {
            "Unknown CPU".to_string()
        };

        SystemSpecs {
            os_name,
            os_version,
            total_memory_gb,
            available_memory_gb,
            cpu_brand,
            cpu_cores: sys.physical_core_count().unwrap_or(0),
            cpu_threads: cpus.len(),
        }
    }

    /// Check if the system meets minimum requirements for local LLMs
    pub fn check_compatibility(&self) -> (bool, Vec<String>) {
        let mut warnings = Vec::new();
        let mut is_compatible = true;

        if self.total_memory_gb < 4 {
            warnings.push("Limited RAM detected. Aide will suggest extremely small models.".to_string());
        } else if self.total_memory_gb < 8 {
            warnings.push("8GB+ RAM is recommended for better quality models.".to_string());
        }

        if self.cpu_cores < 2 {
            warnings.push("Single-core CPU detected. Performance may be very slow.".to_string());
        }

        if self.total_memory_gb < 2 {
            is_compatible = false;
        }

        (is_compatible, warnings)
    }
}
