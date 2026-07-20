use crate::{python_types::create_nexdl_module, AddonApiError, Result};
use dashmap::DashMap;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info, warn};

/// Metadata about a loaded addon
#[derive(Debug, Clone)]
pub struct AddonInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub supported_sites: Vec<String>,
    pub path: PathBuf,
    pub enabled: bool,
}

/// Registry of all loaded Python addons
pub struct AddonRegistry {
    addons: Arc<DashMap<String, AddonInfo>>,
    /// name -> Python class object (Py<PyAny>)
    classes: Arc<DashMap<String, PyObject>>,
    pub addon_dir: PathBuf,
}

impl AddonRegistry {
    pub fn new(addon_dir: PathBuf) -> Self {
        Self {
            addons: Arc::new(DashMap::new()),
            classes: Arc::new(DashMap::new()),
            addon_dir,
        }
    }

    /// Initialize Python interpreter and register the `nexdl` built-in module
    pub fn init_python(&self) -> Result<()> {
        Python::with_gil(|py| {
            // Add addon dir to sys.path
            let sys = py.import_bound("sys")?;
            let path = sys.getattr("path")?;
            let addon_path = self.addon_dir.to_str().unwrap_or(".");
            path.call_method1("insert", (0, addon_path))?;

            // Register built-in nexdl module
            let nexdl_mod = create_nexdl_module(py)?;
            let modules = sys.getattr("modules")?;
            modules.set_item("nexdl", nexdl_mod)?;

            info!("Python {} initialized, nexdl module registered", py.version());
            Ok(())
        }).map_err(|e: PyErr| AddonApiError::Python(e.to_string()))
    }

    /// Scan addon_dir and load all addons
    pub fn load_all(&self) -> Result<usize> {
        let mut count = 0;
        if !self.addon_dir.exists() {
            std::fs::create_dir_all(&self.addon_dir)
                .map_err(|e| AddonApiError::Load(e.to_string()))?;
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.addon_dir)
            .map_err(|e| AddonApiError::Load(e.to_string()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            let is_package = path.is_dir() && path.join("__init__.py").exists();
            let is_module = path.is_file() && path.extension().map(|e| e == "py").unwrap_or(false);

            if is_package || is_module {
                match self.load_addon(&path) {
                    Ok(name) => { info!("Loaded addon: {}", name); count += 1; }
                    Err(e) => error!("Failed to load addon {:?}: {}", path, e),
                }
            }
        }
        Ok(count)
    }

    /// Load a single addon from a path
    pub fn load_addon(&self, path: &PathBuf) -> Result<String> {
        Python::with_gil(|py| {
            let module_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid addon path"))?
                .to_string();

            // Import the addon module
            let module = py.import_bound(module_name.as_str())
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyImportError, _>(
                    format!("Import '{}': {}", module_name, e)
                ))?;

            // Get nexdl.Addon base class
            let nexdl = py.import_bound("nexdl")?;
            let addon_base = nexdl.getattr("Addon")?;

            // Find addon class (subclass of nexdl.Addon)
            let addon_class = self.find_addon_class(py, &module, &addon_base)
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    format!("No nexdl.Addon subclass found in '{}'", module_name)
                ))?;

            // Instantiate to get metadata
            let instance = addon_class.call0()?;
            let name: String = instance.call_method0("name")?.extract()?;
            let version: String = instance.call_method0("version")?.extract()?;
            let description: String = instance.call_method0("description")?.extract()?;
            let author: String = instance.call_method0("author")?.extract()?;
            let supported_sites: Vec<String> = instance.call_method0("supported_sites")?.extract()?;

            let info = AddonInfo { name: name.clone(), version, description, author, supported_sites, path: path.clone(), enabled: true };
            self.addons.insert(name.clone(), info);
            self.classes.insert(name.clone(), addon_class.unbind());

            Ok(name)
        }).map_err(|e: PyErr| AddonApiError::Python(e.to_string()))
    }

    /// Find which addon can handle a URL
    pub fn find_for_url(&self, url: &str) -> Option<String> {
        Python::with_gil(|py| {
            for entry in self.classes.iter() {
                let name = entry.key().clone();
                if let Some(info) = self.addons.get(&name) {
                    if !info.enabled { continue; }
                }
                let class = entry.value().bind(py);
                if let Ok(instance) = class.call0() {
                    if let Ok(result) = instance.call_method1("can_handle", (url,)) {
                        if result.is_truthy().unwrap_or(false) {
                            return Some(name);
                        }
                    }
                }
            }
            None
        })
    }

    /// Create a fresh instance of an addon
    pub fn instantiate(&self, name: &str) -> Result<PyObject> {
        Python::with_gil(|py| {
            self.classes
                .get(name)
                .ok_or_else(|| AddonApiError::NotFound(name.to_string()))
                .and_then(|class| {
                    class.bind(py).call0()
                        .map(|obj| obj.unbind())
                        .map_err(|e| AddonApiError::Python(e.to_string()))
                })
        })
    }

    pub fn list(&self) -> Vec<AddonInfo> {
        self.addons.iter().map(|e| e.clone()).collect()
    }

    pub fn get_info(&self, name: &str) -> Option<AddonInfo> {
        self.addons.get(name).map(|e| e.clone())
    }

    pub fn enable(&self, name: &str, enabled: bool) {
        if let Some(mut info) = self.addons.get_mut(name) {
            info.enabled = enabled;
        }
    }

    pub fn reload(&self, name: &str) -> Result<()> {
        let path = self.addons.get(name)
            .map(|i| i.path.clone())
            .ok_or_else(|| AddonApiError::NotFound(name.to_string()))?;
        self.addons.remove(name);
        self.classes.remove(name);
        self.load_addon(&path)?;
        Ok(())
    }

    fn find_addon_class<'py>(
        &self,
        py: Python<'py>,
        module: &Bound<'py, PyModule>,
        addon_base: &Bound<'py, PyAny>,
    ) -> Option<Bound<'py, PyAny>> {
        let builtins = py.import_bound("builtins").ok()?;
        let dir = module.dir().ok()?;
        for attr_name in dir.iter() {
            let attr_name: String = attr_name.extract().ok()?;
            if attr_name.starts_with('_') { continue; }
            if let Ok(obj) = module.getattr(attr_name.as_str()) {
                if let Ok(result) = builtins.call_method1("issubclass", (&obj, addon_base)) {
                    if result.is_truthy().ok()? && !obj.is(addon_base) {
                        return Some(obj);
                    }
                }
            }
        }
        None
    }
}
