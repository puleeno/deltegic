use pyo3::prelude::*;
use std::collections::HashMap;
use crate::ui_types::*;

// ── UIForm ────────────────────────────────────────────────────────────────────

#[pyclass(name = "UIForm", subclass)]
pub struct PyUIForm {
    id: String,
    #[pyo3(get)]
    pub title: String,
    children: Vec<Widget>,
}

#[pymethods]
impl PyUIForm {
    #[new]
    #[pyo3(signature = (title="",))]
    fn new(title: &str) -> Self {
        Self {
            id: format!("form_{}", rand_id()),
            title: title.to_string(),
            children: Vec::new(),
        }
    }

    #[pyo3(signature = (id, label, placeholder="", input_type="text"))]
    fn add_input(&mut self, id: &str, label: &str, placeholder: &str, input_type: &str) {
        self.children.push(Widget::Input(InputWidget {
            id: id.to_string(),
            label: label.to_string(),
            placeholder: placeholder.to_string(),
            input_type: input_type.to_string(),
            value: String::new(),
            width: 0.0,
        }));
    }

    #[pyo3(signature = (id, label, placeholder="", height=120.0))]
    fn add_textarea(&mut self, id: &str, label: &str, placeholder: &str, height: f32) {
        self.children.push(Widget::Textarea(TextareaWidget {
            id: id.to_string(),
            label: label.to_string(),
            placeholder: placeholder.to_string(),
            value: String::new(),
            height,
        }));
    }

    #[pyo3(signature = (id, label, default=false))]
    fn add_checkbox(&mut self, id: &str, label: &str, default: bool) {
        self.children.push(Widget::Checkbox(CheckboxWidget {
            id: id.to_string(),
            label: label.to_string(),
            checked: default,
        }));
    }

    #[pyo3(signature = (id, label, options, selected_index=0))]
    fn add_select(&mut self, id: &str, label: &str, options: Vec<String>, selected_index: usize) {
        self.children.push(Widget::Select(SelectWidget {
            id: id.to_string(),
            label: label.to_string(),
            options,
            selected_index,
        }));
    }

    #[pyo3(signature = (id, label, style=""))]
    fn add_button(&mut self, id: &str, label: &str, style: &str) {
        self.children.push(Widget::Button(ButtonWidget {
            id: id.to_string(),
            label: label.to_string(),
            style: style.to_string(),
        }));
    }

    #[pyo3(signature = (id, text, size=14.0))]
    fn add_text(&mut self, id: &str, text: &str, size: f32) {
        self.children.push(Widget::Text(TextWidget {
            id: id.to_string(),
            text: text.to_string(),
            size,
            style: String::new(),
        }));
    }

    #[pyo3(signature = (id, label="", value=0.0, max=100.0))]
    fn add_progress(&mut self, id: &str, label: &str, value: f32, max: f32) {
        self.children.push(Widget::Progress(ProgressWidget {
            id: id.to_string(),
            label: label.to_string(),
            value,
            max,
        }));
    }

    #[pyo3(signature = (height=8.0,))]
    fn add_spacer(&mut self, height: f32) {
        self.children.push(Widget::Spacer(SpacerWidget { height }));
    }

    fn get_widget_tree(&self) -> PyResult<String> {
        let form = FormWidget {
            id: self.id.clone(),
            title: self.title.clone(),
            children: self.children.clone(),
        };
        serde_json::to_string(&Widget::Form(form))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }
}

// ── UIPanel ───────────────────────────────────────────────────────────────────

#[pyclass(name = "UIPanel", subclass)]
pub struct PyUIPanel {
    id: String,
    #[pyo3(get)]
    pub title: String,
    children: Vec<Widget>,
}

#[pymethods]
impl PyUIPanel {
    #[new]
    #[pyo3(signature = (title="",))]
    fn new(title: &str) -> Self {
        Self {
            id: format!("panel_{}", rand_id()),
            title: title.to_string(),
            children: Vec::new(),
        }
    }

    #[pyo3(signature = (id, label="", value=0.0, max=100.0))]
    fn add_progress(&mut self, id: &str, label: &str, value: f32, max: f32) {
        self.children.push(Widget::Progress(ProgressWidget {
            id: id.to_string(),
            label: label.to_string(),
            value,
            max,
        }));
    }

    #[pyo3(signature = (id, text, size=14.0))]
    fn add_text(&mut self, id: &str, text: &str, size: f32) {
        self.children.push(Widget::Text(TextWidget {
            id: id.to_string(),
            text: text.to_string(),
            size,
            style: String::new(),
        }));
    }

    #[pyo3(signature = (id, label="", height=200.0))]
    fn add_log(&mut self, id: &str, label: &str, height: f32) {
        self.children.push(Widget::Log(LogWidget {
            id: id.to_string(),
            label: label.to_string(),
            lines: Vec::new(),
            height,
        }));
    }

    #[pyo3(signature = (id, label, placeholder=""))]
    fn add_input(&mut self, id: &str, label: &str, placeholder: &str) {
        self.children.push(Widget::Input(InputWidget {
            id: id.to_string(),
            label: label.to_string(),
            placeholder: placeholder.to_string(),
            input_type: "text".to_string(),
            value: String::new(),
            width: 0.0,
        }));
    }

    #[pyo3(signature = (id, label, style=""))]
    fn add_button(&mut self, id: &str, label: &str, style: &str) {
        self.children.push(Widget::Button(ButtonWidget {
            id: id.to_string(),
            label: label.to_string(),
            style: style.to_string(),
        }));
    }

    #[pyo3(signature = (height=8.0,))]
    fn add_spacer(&mut self, height: f32) {
        self.children.push(Widget::Spacer(SpacerWidget { height }));
    }

    fn get_widget_tree(&self) -> PyResult<String> {
        let panel = PanelWidget {
            id: self.id.clone(),
            title: self.title.clone(),
            children: self.children.clone(),
        };
        serde_json::to_string(&Widget::Panel(panel))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }
}

// ── UIWindow ──────────────────────────────────────────────────────────────────

#[pyclass(name = "UIWindow", subclass)]
pub struct PyUIWindow {
    id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub width: f32,
    #[pyo3(get)]
    pub height: f32,
    #[pyo3(get)]
    pub always_on_top: bool,
    children: Vec<Widget>,
    last_form: Option<FormWidget>,
    last_panel: Option<PanelWidget>,
}

#[pymethods]
impl PyUIWindow {
    #[new]
    #[pyo3(signature = (title, width=600.0, height=400.0, always_on_top=false))]
    fn new(title: &str, width: f32, height: f32, always_on_top: bool) -> Self {
        Self {
            id: format!("window_{}", rand_id()),
            title: title.to_string(),
            width,
            height,
            always_on_top,
            children: Vec::new(),
            last_form: None,
            last_panel: None,
        }
    }

    fn create_form(&mut self, title: &str) {
        self.last_form = Some(FormWidget {
            id: format!("form_{}", rand_id()),
            title: title.to_string(),
            children: Vec::new(),
        });
    }

    fn create_panel(&mut self, title: &str) {
        self.last_panel = Some(PanelWidget {
            id: format!("panel_{}", rand_id()),
            title: title.to_string(),
            children: Vec::new(),
        });
    }

    fn finish_form(&mut self) {
        if let Some(form) = self.last_form.take() {
            self.children.push(Widget::Form(form));
        }
    }

    fn finish_panel(&mut self) {
        if let Some(panel) = self.last_panel.take() {
            self.children.push(Widget::Panel(panel));
        }
    }

    #[pyo3(signature = (id, label, placeholder="", input_type="text"))]
    fn form_add_input(&mut self, id: &str, label: &str, placeholder: &str, input_type: &str) {
        if let Some(ref mut form) = self.last_form {
            form.children.push(Widget::Input(InputWidget {
                id: id.to_string(),
                label: label.to_string(),
                placeholder: placeholder.to_string(),
                input_type: input_type.to_string(),
                value: String::new(),
                width: 0.0,
            }));
        }
    }

    #[pyo3(signature = (id, label, placeholder="", height=120.0))]
    fn form_add_textarea(&mut self, id: &str, label: &str, placeholder: &str, height: f32) {
        if let Some(ref mut form) = self.last_form {
            form.children.push(Widget::Textarea(TextareaWidget {
                id: id.to_string(),
                label: label.to_string(),
                placeholder: placeholder.to_string(),
                value: String::new(),
                height,
            }));
        }
    }

    #[pyo3(signature = (id, label, default=false))]
    fn form_add_checkbox(&mut self, id: &str, label: &str, default: bool) {
        if let Some(ref mut form) = self.last_form {
            form.children.push(Widget::Checkbox(CheckboxWidget {
                id: id.to_string(),
                label: label.to_string(),
                checked: default,
            }));
        }
    }

    #[pyo3(signature = (id, label, options, selected_index=0))]
    fn form_add_select(&mut self, id: &str, label: &str, options: Vec<String>, selected_index: usize) {
        if let Some(ref mut form) = self.last_form {
            form.children.push(Widget::Select(SelectWidget {
                id: id.to_string(),
                label: label.to_string(),
                options,
                selected_index,
            }));
        }
    }

    #[pyo3(signature = (id, label, style=""))]
    fn form_add_button(&mut self, id: &str, label: &str, style: &str) {
        if let Some(ref mut form) = self.last_form {
            form.children.push(Widget::Button(ButtonWidget {
                id: id.to_string(),
                label: label.to_string(),
                style: style.to_string(),
            }));
        }
    }

    #[pyo3(signature = (id, label="", value=0.0, max=100.0))]
    fn panel_add_progress(&mut self, id: &str, label: &str, value: f32, max: f32) {
        if let Some(ref mut panel) = self.last_panel {
            panel.children.push(Widget::Progress(ProgressWidget {
                id: id.to_string(),
                label: label.to_string(),
                value,
                max,
            }));
        }
    }

    #[pyo3(signature = (id, text, size=14.0))]
    fn panel_add_text(&mut self, id: &str, text: &str, size: f32) {
        if let Some(ref mut panel) = self.last_panel {
            panel.children.push(Widget::Text(TextWidget {
                id: id.to_string(),
                text: text.to_string(),
                size,
                style: String::new(),
            }));
        }
    }

    #[pyo3(signature = (id, label="", height=200.0))]
    fn panel_add_log(&mut self, id: &str, label: &str, height: f32) {
        if let Some(ref mut panel) = self.last_panel {
            panel.children.push(Widget::Log(LogWidget {
                id: id.to_string(),
                label: label.to_string(),
                lines: Vec::new(),
                height,
            }));
        }
    }

    #[pyo3(signature = (id, text, size=14.0))]
    fn add_text(&mut self, id: &str, text: &str, size: f32) {
        self.children.push(Widget::Text(TextWidget {
            id: id.to_string(),
            text: text.to_string(),
            size,
            style: String::new(),
        }));
    }

    #[pyo3(signature = (id, label, style=""))]
    fn add_button(&mut self, id: &str, label: &str, style: &str) {
        self.children.push(Widget::Button(ButtonWidget {
            id: id.to_string(),
            label: label.to_string(),
            style: style.to_string(),
        }));
    }

    #[pyo3(signature = (height=8.0,))]
    fn add_spacer(&mut self, height: f32) {
        self.children.push(Widget::Spacer(SpacerWidget { height }));
    }

    fn get_widget_tree(&self) -> PyResult<String> {
        let window = WindowWidget {
            id: self.id.clone(),
            title: self.title.clone(),
            width: self.width,
            height: self.height,
            always_on_top: self.always_on_top,
            children: self.children.clone(),
            properties: HashMap::new(),
        };
        serde_json::to_string(&Widget::Window(window))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }
}

// ── UITabs ────────────────────────────────────────────────────────────────────

#[pyclass(name = "UITabs", subclass)]
pub struct PyUITabs {
    id: String,
    tabs: Vec<TabPage>,
    current_tab: Option<TabBuilder>,
}

struct TabBuilder {
    label: String,
    children: Vec<Widget>,
}

#[pymethods]
impl PyUITabs {
    #[new]
    fn new() -> Self {
        Self {
            id: format!("tabs_{}", rand_id()),
            tabs: Vec::new(),
            current_tab: None,
        }
    }

    fn begin_tab(&mut self, label: &str) {
        self.current_tab = Some(TabBuilder {
            label: label.to_string(),
            children: Vec::new(),
        });
    }

    #[pyo3(signature = (id, label, placeholder=""))]
    fn add_input(&mut self, id: &str, label: &str, placeholder: &str) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Input(InputWidget {
                id: id.to_string(),
                label: label.to_string(),
                placeholder: placeholder.to_string(),
                input_type: "text".to_string(),
                value: String::new(),
                width: 0.0,
            }));
        }
    }

    #[pyo3(signature = (id, label, placeholder="", height=120.0))]
    fn add_textarea(&mut self, id: &str, label: &str, placeholder: &str, height: f32) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Textarea(TextareaWidget {
                id: id.to_string(),
                label: label.to_string(),
                placeholder: placeholder.to_string(),
                value: String::new(),
                height,
            }));
        }
    }

    #[pyo3(signature = (id, label, default=false))]
    fn add_checkbox(&mut self, id: &str, label: &str, default: bool) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Checkbox(CheckboxWidget {
                id: id.to_string(),
                label: label.to_string(),
                checked: default,
            }));
        }
    }

    #[pyo3(signature = (id, label, options))]
    fn add_select(&mut self, id: &str, label: &str, options: Vec<String>) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Select(SelectWidget {
                id: id.to_string(),
                label: label.to_string(),
                options,
                selected_index: 0,
            }));
        }
    }

    #[pyo3(signature = (id, label))]
    fn add_button(&mut self, id: &str, label: &str) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Button(ButtonWidget {
                id: id.to_string(),
                label: label.to_string(),
                style: String::new(),
            }));
        }
    }

    #[pyo3(signature = (id, text, size=14.0))]
    fn add_text(&mut self, id: &str, text: &str, size: f32) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Text(TextWidget {
                id: id.to_string(),
                text: text.to_string(),
                size,
                style: String::new(),
            }));
        }
    }

    #[pyo3(signature = (id, label="", value=0.0, max=100.0))]
    fn add_progress(&mut self, id: &str, label: &str, value: f32, max: f32) {
        if let Some(ref mut tab) = self.current_tab {
            tab.children.push(Widget::Progress(ProgressWidget {
                id: id.to_string(),
                label: label.to_string(),
                value,
                max,
            }));
        }
    }

    fn end_tab(&mut self) {
        if let Some(tab) = self.current_tab.take() {
            self.tabs.push(TabPage {
                label: tab.label,
                children: tab.children,
            });
        }
    }

    fn get_widget_tree(&self) -> PyResult<String> {
        let tabs = TabsWidget {
            id: self.id.clone(),
            tabs: self.tabs.clone(),
        };
        serde_json::to_string(&Widget::Tabs(tabs))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))
    }
}

// ── UIContext ─────────────────────────────────────────────────────────────────

#[pyclass(name = "UIContext", subclass)]
pub struct PyUIContext {
    widget_values: std::sync::Arc<parking_lot::Mutex<HashMap<String, String>>>,
}

impl PyUIContext {
    pub fn new() -> Self {
        Self {
            widget_values: std::sync::Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }
}

#[pymethods]
impl PyUIContext {
    #[new]
    fn create() -> Self {
        Self::new()
    }

    fn get_value(&self, id: &str) -> String {
        self.widget_values.lock().get(id).cloned().unwrap_or_default()
    }

    fn set_value(&self, id: &str, value: &str) {
        self.widget_values.lock().insert(id.to_string(), value.to_string());
    }

    fn get_all_values(&self) -> HashMap<String, String> {
        self.widget_values.lock().clone()
    }
}

fn rand_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{:x}", t.as_nanos())
}
