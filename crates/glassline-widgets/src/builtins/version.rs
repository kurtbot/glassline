//! `version` — Claude Code version string. Port of upstream `Version.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(VersionWidget)
}

pub struct VersionWidget;

impl Widget for VersionWidget {
    fn id(&self) -> &'static str {
        "version"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(v) = ctx
            .data
            .as_ref()
            .and_then(|d| d.version.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, labeled_or_raw(spec, "v", v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::StatusJson;

    fn ctx(version: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                version: version.map(String::from),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_version_with_v_prefix() {
        let spans = VersionWidget.render(&WidgetSpec::new("1", "version"), &ctx(Some("1.2.3")));
        assert_eq!(spans[0].text, "v1.2.3");
    }

    #[test]
    fn raw_drops_prefix() {
        let mut spec = WidgetSpec::new("1", "version");
        spec.raw_value = Some(true);
        let spans = VersionWidget.render(&spec, &ctx(Some("1.2.3")));
        assert_eq!(spans[0].text, "1.2.3");
    }

    #[test]
    fn empty_when_version_absent() {
        let spans = VersionWidget.render(&WidgetSpec::new("1", "version"), &ctx(None));
        assert!(spans.is_empty());
    }
}
