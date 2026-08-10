//! PyO3 bindings — the `nexdl` Python module exposed to addon authors.
//!
//! Python addon API:
//!
//! ```python
//! import nexdl
//!
//! class MyAddon(nexdl.Addon):
//!     def name(self) -> str: return "my-addon"
//!     def version(self) -> str: return "1.0.0"
//!     def can_handle(self, url: str) -> bool: return "mysite.com" in url
//!     def extract(self, ctx: nexdl.Context) -> list[nexdl.DownloadItem]: ...
//!     def download(self, item: nexdl.DownloadItem, ctx: nexdl.Context) -> None: ...
//!     def post_process(self, item: nexdl.DownloadItem, ctx: nexdl.Context) -> None: ...
//! ```

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::HashMap;

// ── DownloadItem ──────────────────────────────────────────────────────────────

#[pyclass(name = "DownloadItem")]
#[derive(Clone, Debug)]
pub struct PyDownloadItem {
    #[pyo3(get, set)] pub url: String,
    #[pyo3(get, set)] pub title: String,
    #[pyo3(get, set)] pub output_path: String,
    #[pyo3(get, set)] pub filename: String,
    #[pyo3(get, set)] pub size: i64,
    #[pyo3(get, set)] pub mime_type: String,
    #[pyo3(get, set)] pub referer: String,
    #[pyo3(get, set)] pub cookies: String,
    #[pyo3(get, set)] pub headers: HashMap<String, String>,
    #[pyo3(get, set)] pub metadata: HashMap<String, String>,
    #[pyo3(get, set)] pub index: i32,
    #[pyo3(get, set)] pub total: i32,
}

#[pymethods]
impl PyDownloadItem {
    #[new]
    #[pyo3(signature = (url, title="", output_path="", filename=""))]
    pub fn new(url: &str, title: &str, output_path: &str, filename: &str) -> Self {
        Self {
            url: url.to_string(),
            title: title.to_string(),
            output_path: output_path.to_string(),
            filename: if filename.is_empty() {
                url.split('/').last().unwrap_or("download").to_string()
            } else {
                filename.to_string()
            },
            size: -1,
            mime_type: String::new(),
            referer: String::new(),
            cookies: String::new(),
            headers: HashMap::new(),
            metadata: HashMap::new(),
            index: 0,
            total: 1,
        }
    }

    fn __repr__(&self) -> String {
        format!("DownloadItem(title={:?}, url={:?})", self.title, self.url)
    }
}

// ── HttpClient ────────────────────────────────────────────────────────────────

#[pyclass(name = "HttpClient")]
pub struct PyHttpClient {
    #[pyo3(get, set)] pub cookies: String,
    pub headers: HashMap<String, String>,
}

impl PyHttpClient {
    pub fn create() -> Self {
        Self {
            cookies: String::new(),
            headers: HashMap::new(),
        }
    }
}

#[pymethods]
impl PyHttpClient {
    #[new]
    pub fn new() -> Self {
        Self::create()
    }

    pub fn set_cookie(&mut self, cookie: &str) {
        self.cookies = cookie.to_string();
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }

    /// GET → text
    pub fn get(&self, url: &str) -> PyResult<String> {
        let cookies = self.cookies.clone();
        let headers = self.headers.clone();
        blocking_get(url, cookies, headers)
    }

    /// GET → Python dict/list (JSON)
    pub fn get_json(&self, url: &str, py: Python<'_>) -> PyResult<PyObject> {
        let text = self.get(url)?;
        let val: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        json_to_py(py, &val)
    }

    /// POST JSON body → text
    pub fn post_json(&self, url: &str, body: &str) -> PyResult<String> {
        let body_val: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let cookies = self.cookies.clone();
        let headers = self.headers.clone();
        blocking_post_json(url, body_val, cookies, headers)
    }

    /// Download URL to file, returns bytes written
    pub fn download(&self, url: &str, dest: &str) -> PyResult<u64> {
        let cookies = self.cookies.clone();
        let headers = self.headers.clone();
        let dest_str = dest.to_string();
        let url_str = url.to_string();

        // Run in a new thread with its own tokio runtime to avoid nested-runtime issues
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

            rt.block_on(async move {
                let dest_path = std::path::PathBuf::from(&dest_str);
                if let Some(parent) = dest_path.parent() {
                    tokio::fs::create_dir_all(parent).await
                        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
                }

                let client_builder = reqwest::Client::builder()
                    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

                let client = client_builder.build()
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;

                let mut req = client.get(&url_str);
                if !cookies.is_empty() { req = req.header("Cookie", &cookies); }
                for (k, v) in &headers { req = req.header(k.as_str(), v.as_str()); }

                let bytes = req.send().await
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?
                    .bytes().await
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;

                tokio::fs::write(&dest_path, &bytes).await
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;

                Ok::<u64, PyErr>(bytes.len() as u64)
            })
        }).join().map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Thread panicked"))?
    }
}

fn blocking_get(url: &str, cookies: String, headers: HashMap<String, String>) -> PyResult<String> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        rt.block_on(async move {
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .build()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
            let mut req = client.get(&url);
            if !cookies.is_empty() { req = req.header("Cookie", &cookies); }
            for (k, v) in &headers { req = req.header(k.as_str(), v.as_str()); }
            req.send().await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?
                .text().await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
        })
    }).join().map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Thread panicked"))?
}

fn blocking_post_json(
    url: &str,
    body: serde_json::Value,
    cookies: String,
    headers: HashMap<String, String>,
) -> PyResult<String> {
    let url = url.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        rt.block_on(async move {
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0")
                .build()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
            let mut req = client.post(&url).json(&body);
            if !cookies.is_empty() { req = req.header("Cookie", &cookies); }
            for (k, v) in &headers { req = req.header(k.as_str(), v.as_str()); }
            req.send().await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?
                .text().await
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
        })
    }).join().map_err(|_| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("Thread panicked"))?
}

// ── Storage ───────────────────────────────────────────────────────────────────

#[pyclass(name = "Storage")]
pub struct PyStorage {
    #[pyo3(get)] pub base_dir: String,
}

#[pymethods]
impl PyStorage {
    #[new]
    pub fn new(base_dir: &str) -> Self {
        Self { base_dir: base_dir.to_string() }
    }

    pub fn read_text(&self, path: &str) -> PyResult<String> {
        let full = std::path::Path::new(&self.base_dir).join(path);
        std::fs::read_to_string(full)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
    }

    pub fn write_text(&self, path: &str, content: &str) -> PyResult<()> {
        let full = std::path::Path::new(&self.base_dir).join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        }
        std::fs::write(full, content)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
    }

    pub fn read_json(&self, path: &str, py: Python<'_>) -> PyResult<PyObject> {
        let text = self.read_text(path)?;
        let val: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        json_to_py(py, &val)
    }

    pub fn write_json(&self, path: &str, json_str: &str) -> PyResult<()> {
        self.write_text(path, json_str)
    }

    pub fn exists(&self, path: &str) -> bool {
        std::path::Path::new(&self.base_dir).join(path).exists()
    }

    pub fn list_dir(&self, path: &str) -> PyResult<Vec<String>> {
        let full = std::path::Path::new(&self.base_dir).join(path);
        let entries = std::fs::read_dir(full)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        Ok(entries.flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect())
    }
}

// ── Context ───────────────────────────────────────────────────────────────────

#[pyclass(name = "Context")]
pub struct PyContext {
    #[pyo3(get, set)] pub url: String,
    #[pyo3(get, set)] pub output_dir: String,
    #[pyo3(get, set)] pub options: HashMap<String, String>,
    http_inner: PyHttpClient,
    storage_inner: PyStorage,
}

impl PyContext {
    pub fn create(url: String, output_dir: String) -> Self {
        Self {
            url: url.clone(),
            output_dir: output_dir.clone(),
            options: HashMap::new(),
            http_inner: PyHttpClient::create(),
            storage_inner: PyStorage::new(&output_dir),
        }
    }

    pub fn set_cookies(&mut self, cookies: &str) {
        self.http_inner.cookies = cookies.to_string();
    }

    pub fn http_mut(&mut self) -> &mut PyHttpClient {
        &mut self.http_inner
    }
}

#[pymethods]
impl PyContext {
    #[new]
    pub fn new(url: &str, output_dir: &str) -> Self {
        Self::create(url.to_string(), output_dir.to_string())
    }

    #[getter]
    pub fn http(&self, py: Python<'_>) -> PyResult<PyObject> {
        // Return a reference proxy — simplified: return a new PyHttpClient with same state
        let client = PyHttpClient {
            cookies: self.http_inner.cookies.clone(),
            headers: self.http_inner.headers.clone(),
        };
        Ok(Py::new(py, client)?.into_any())
    }

    #[getter]
    pub fn storage(&self, py: Python<'_>) -> PyResult<PyObject> {
        let s = PyStorage::new(&self.storage_inner.base_dir);
        Ok(Py::new(py, s)?.into_any())
    }

    pub fn report_progress(&self, downloaded: u64, total: u64, message: &str) {
        tracing::debug!("[addon progress] {}/{} {}", downloaded, total, message);
    }

    pub fn log(&self, level: &str, message: &str) {
        match level {
            "debug" => tracing::debug!("[addon] {}", message),
            "warn" | "warning" => tracing::warn!("[addon] {}", message),
            "error" => tracing::error!("[addon] {}", message),
            _ => tracing::info!("[addon] {}", message),
        }
    }

    pub fn get_option(&self, key: &str, default: &str) -> String {
        self.options.get(key).cloned().unwrap_or_else(|| default.to_string())
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }
}

// ── Addon base class ──────────────────────────────────────────────────────────

#[pyclass(name = "Addon", subclass)]
pub struct PyAddonBase {}

#[pymethods]
impl PyAddonBase {
    #[new]
    pub fn new() -> Self { Self {} }

    pub fn name(&self) -> &str { "unnamed-addon" }
    pub fn version(&self) -> &str { "0.0.1" }
    pub fn description(&self) -> &str { "" }
    pub fn author(&self) -> &str { "" }
    pub fn supported_sites(&self) -> Vec<String> { Vec::new() }
    pub fn can_handle(&self, _url: &str) -> bool { false }

    pub fn extract(&self, _ctx: &PyContext) -> PyResult<Vec<PyDownloadItem>> {
        Ok(Vec::new())
    }

    pub fn download(&self, item: &PyDownloadItem, ctx: &PyContext) -> PyResult<()> {
        ctx.http_inner.download(&item.url, &item.output_path)?;
        Ok(())
    }

    pub fn post_process(&self, _item: &PyDownloadItem, _ctx: &PyContext) -> PyResult<()> {
        Ok(())
    }

    pub fn resolve_captcha(&self, _ctx: &PyContext, _captcha_url: &str) -> PyResult<String> {
        Err(PyErr::new::<pyo3::exceptions::PyNotImplementedError, _>(
            "Captcha resolution not implemented"
        ))
    }
}

// ── nexdl module factory ──────────────────────────────────────────────────────

pub fn create_nexdl_module(py: Python<'_>) -> PyResult<Bound<'_, pyo3::types::PyModule>> {
    let m = pyo3::types::PyModule::new_bound(py, "nexdl")?;
    m.add_class::<PyAddonBase>()?;
    m.add_class::<PyDownloadItem>()?;
    m.add_class::<PyHttpClient>()?;
    m.add_class::<PyStorage>()?;
    m.add_class::<PyContext>()?;
    // UI classes
    m.add_class::<crate::py_ui::PyUIForm>()?;
    m.add_class::<crate::py_ui::PyUIPanel>()?;
    m.add_class::<crate::py_ui::PyUIWindow>()?;
    m.add_class::<crate::py_ui::PyUITabs>()?;
    m.add_class::<crate::py_ui::PyUIContext>()?;
    Ok(m)
}

// ── JSON → Python ─────────────────────────────────────────────────────────────

pub fn json_to_py(py: Python<'_>, value: &serde_json::Value) -> PyResult<PyObject> {
    use pyo3::types::PyNone;
    match value {
        serde_json::Value::Null => Ok(PyNone::get_bound(py).into_py(py)),
        serde_json::Value::Bool(b) => Ok(b.into_py(py)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Ok(i.into_py(py)) }
            else if let Some(f) = n.as_f64() { Ok(f.into_py(py)) }
            else { Ok(PyNone::get_bound(py).into_py(py)) }
        }
        serde_json::Value::String(s) => Ok(s.as_str().into_py(py)),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty_bound(py);
            for item in arr { list.append(json_to_py(py, item)?)?; }
            Ok(list.into_py(py))
        }
        serde_json::Value::Object(obj) => {
            let dict = PyDict::new_bound(py);
            for (k, v) in obj { dict.set_item(k, json_to_py(py, v)?)?; }
            Ok(dict.into_py(py))
        }
    }
}
