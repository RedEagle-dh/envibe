use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::config::PortRanges;
use crate::error::{EnvibeError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortRegistry {
    /// Maps "project_name:service_name" to assigned port
    assignments: HashMap<String, u16>,
    /// Set of currently used ports
    #[serde(skip)]
    used_ports: HashSet<u16>,
}

impl PortRegistry {
    pub fn new() -> Self {
        Self {
            assignments: HashMap::new(),
            used_ports: HashSet::new(),
        }
    }

    /// Load the port registry from disk
    pub async fn load(data_dir: &PathBuf) -> Result<Self> {
        let path = data_dir.join("ports.json");
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = fs::read_to_string(&path).await?;
        let mut registry: PortRegistry = serde_json::from_str(&content)?;

        // Rebuild used_ports from assignments
        registry.used_ports = registry.assignments.values().copied().collect();

        Ok(registry)
    }

    /// Save the port registry to disk
    pub async fn save(&self, data_dir: &PathBuf) -> Result<()> {
        let path = data_dir.join("ports.json");
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    /// Get or allocate a port for a service
    pub fn get_or_allocate(
        &mut self,
        project: &str,
        service: &str,
        ranges: &PortRanges,
        service_type: ServiceType,
    ) -> Result<u16> {
        let key = format!("{}:{}", project, service);

        // Check if we already have an assignment
        if let Some(&port) = self.assignments.get(&key) {
            if is_port_available(port) {
                self.used_ports.insert(port);
                return Ok(port);
            }
            // Port is no longer available, need to reallocate
            self.assignments.remove(&key);
            self.used_ports.remove(&port);
        }

        // Allocate a new port
        let (start, end) = self.get_range(ranges, service_type);
        let port = self.find_available_port(start, end)?;

        self.assignments.insert(key, port);
        self.used_ports.insert(port);

        Ok(port)
    }

    /// Release a port assignment
    pub fn release(&mut self, project: &str, service: &str) {
        let key = format!("{}:{}", project, service);
        if let Some(port) = self.assignments.remove(&key) {
            self.used_ports.remove(&port);
        }
    }

    /// Get the port for a service if already assigned
    pub fn get_port(&self, project: &str, service: &str) -> Option<u16> {
        let key = format!("{}:{}", project, service);
        self.assignments.get(&key).copied()
    }

    /// Get all ports for a project as a map of service_name -> port
    pub fn get_project_ports(&self, project: &str) -> Vec<(String, u16)> {
        let prefix = format!("{}:", project);
        self.assignments
            .iter()
            .filter_map(|(key, &port)| {
                if key.starts_with(&prefix) {
                    let service = key.strip_prefix(&prefix)?.to_string();
                    Some((service, port))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Manually set a port for a service (used for user overrides)
    /// Returns error if port is already in use by another service
    pub fn set_port(&mut self, project: &str, service: &str, port: u16) -> Result<()> {
        let key = format!("{}:{}", project, service);

        // Check if port is already assigned to a different service
        for (existing_key, &existing_port) in &self.assignments {
            if existing_port == port && existing_key != &key {
                return Err(EnvibeError::PortAllocation(format!(
                    "Port {} is already assigned to {}",
                    port, existing_key
                )));
            }
        }

        // Check if port is available on the system
        if !is_port_available(port) {
            // Check if it's our own service using it
            if self.assignments.get(&key) != Some(&port) {
                return Err(EnvibeError::PortAllocation(format!(
                    "Port {} is already in use on the system",
                    port
                )));
            }
        }

        // Remove old port from used_ports if we had one
        if let Some(&old_port) = self.assignments.get(&key) {
            self.used_ports.remove(&old_port);
        }

        // Assign new port
        self.assignments.insert(key, port);
        self.used_ports.insert(port);

        Ok(())
    }

    fn get_range(&self, ranges: &PortRanges, service_type: ServiceType) -> (u16, u16) {
        match service_type {
            ServiceType::Postgres => ranges.postgres,
            ServiceType::Redis => ranges.redis,
            ServiceType::Mysql => ranges.mysql,
            ServiceType::Mongo => ranges.mongo,
            ServiceType::Http => ranges.http,
            ServiceType::Generic => ranges.generic,
        }
    }

    fn find_available_port(&self, start: u16, end: u16) -> Result<u16> {
        for port in start..=end {
            if !self.used_ports.contains(&port) && is_port_available(port) {
                return Ok(port);
            }
        }
        Err(EnvibeError::PortAllocation(format!(
            "No available ports in range {}-{}",
            start, end
        )))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ServiceType {
    Postgres,
    Redis,
    Mysql,
    Mongo,
    Http,
    Generic,
}

impl ServiceType {
    pub fn from_image(image: &str) -> Self {
        let image_lower = image.to_lowercase();
        if image_lower.contains("postgres") {
            ServiceType::Postgres
        } else if image_lower.contains("redis") {
            ServiceType::Redis
        } else if image_lower.contains("mysql") || image_lower.contains("mariadb") {
            ServiceType::Mysql
        } else if image_lower.contains("mongo") {
            ServiceType::Mongo
        } else if image_lower.contains("nginx")
            || image_lower.contains("httpd")
            || image_lower.contains("node")
        {
            ServiceType::Http
        } else {
            ServiceType::Generic
        }
    }

    pub fn from_service_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        if name_lower.contains("postgres") || name_lower == "db" || name_lower == "database" {
            ServiceType::Postgres
        } else if name_lower.contains("redis") || name_lower == "cache" {
            ServiceType::Redis
        } else if name_lower.contains("mysql") || name_lower.contains("mariadb") {
            ServiceType::Mysql
        } else if name_lower.contains("mongo") {
            ServiceType::Mongo
        } else if name_lower.contains("web")
            || name_lower.contains("app")
            || name_lower.contains("server")
            || name_lower.contains("api")
        {
            ServiceType::Http
        } else {
            ServiceType::Generic
        }
    }
}

/// Check if a port is available for binding
fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}
