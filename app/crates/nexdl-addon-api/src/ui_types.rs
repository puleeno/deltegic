use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A unique ID for each widget instance
pub type WidgetId = String;

/// Widget tree that can be serialized to JSON and rendered by Slint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Widget {
    Window(WindowWidget),
    Form(FormWidget),
    Panel(PanelWidget),
    Tabs(TabsWidget),
    Input(InputWidget),
    Textarea(TextareaWidget),
    Checkbox(CheckboxWidget),
    Select(SelectWidget),
    Button(ButtonWidget),
    Progress(ProgressWidget),
    Text(TextWidget),
    Log(LogWidget),
    Table(TableWidget),
    Spacer(SpacerWidget),
    HorizontalLayout(HorizontalLayoutWidget),
    VerticalLayout(VerticalLayoutWidget),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowWidget {
    pub id: WidgetId,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub always_on_top: bool,
    pub children: Vec<Widget>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormWidget {
    pub id: WidgetId,
    pub title: String,
    #[serde(default)]
    pub children: Vec<Widget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub children: Vec<Widget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabsWidget {
    pub id: WidgetId,
    pub tabs: Vec<TabPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabPage {
    pub label: String,
    pub children: Vec<Widget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default = "default_input_type")]
    pub input_type: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub width: f32,
}

fn default_input_type() -> String {
    "text".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextareaWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub value: String,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckboxWidget {
    pub id: WidgetId,
    pub label: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectWidget {
    pub id: WidgetId,
    pub label: String,
    pub options: Vec<String>,
    #[serde(default)]
    pub selected_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonWidget {
    pub id: WidgetId,
    pub label: String,
    #[serde(default)]
    pub style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub label: String,
    pub value: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextWidget {
    pub id: WidgetId,
    pub text: String,
    #[serde(default = "default_text_size")]
    pub size: f32,
    #[serde(default)]
    pub style: String,
}

fn default_text_size() -> f32 {
    14.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub lines: Vec<String>,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableWidget {
    pub id: WidgetId,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacerWidget {
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HorizontalLayoutWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub spacing: f32,
    pub children: Vec<Widget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerticalLayoutWidget {
    pub id: WidgetId,
    #[serde(default)]
    pub spacing: f32,
    pub children: Vec<Widget>,
}

/// Builder for constructing widget trees imperatively
pub struct WidgetTreeBuilder {
    pub root: Vec<Widget>,
}

impl WidgetTreeBuilder {
    pub fn new() -> Self {
        Self {
            root: Vec::new(),
        }
    }

    pub fn build(self) -> Vec<Widget> {
        self.root
    }
}
