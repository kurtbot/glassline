//! Editor screens. P2 lands only the [`widget_picker`] screen; the
//! full screen tree (LineListEditor, ItemsEditor, WidgetEditor, …)
//! arrives in P3.

pub mod widget_picker;

pub use widget_picker::WidgetPicker;
