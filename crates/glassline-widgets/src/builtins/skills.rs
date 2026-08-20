//! `skills` — a summary of which skills have been invoked this session.
//! Port of upstream `Skills.tsx`.
//!
//! Reads `ctx.skills_metrics.skills_invoked` — a map of `name -> count`
//! that the render binary populates from the transcript scanner. Renders
//! as `Skills: alpha×3, beta×1` (comma-separated, sorted by count
//! descending then by name). Empty when nothing has been invoked.
//!
//! Configurable via `metadata.limit` (u32, default 5): caps the number
//! of skills shown. Excess collapse to `+N` suffix.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

const DEFAULT_LIMIT: usize = 5;

pub fn factory() -> Box<dyn Widget> {
    Box::new(SkillsWidget)
}

pub struct SkillsWidget;

impl Widget for SkillsWidget {
    fn id(&self) -> &'static str {
        "skills"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::SKILLS
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightMagenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(m) = ctx.skills_metrics.as_ref() else {
            return Vec::new();
        };
        if m.skills_invoked.is_empty() {
            return Vec::new();
        }
        let limit = spec
            .metadata
            .as_ref()
            .and_then(|md| md.get("limit"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_LIMIT)
            .max(1);

        let mut entries: Vec<(&String, &u64)> = m.skills_invoked.iter().collect();
        // Sort by count desc, then name asc.
        entries.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let total = entries.len();
        let shown: Vec<String> = entries
            .iter()
            .take(limit)
            .map(|(name, count)| format!("{name}\u{00d7}{count}")) // ×
            .collect();
        let mut body = shown.join(", ");
        if total > limit {
            body.push_str(&format!(" +{}", total - limit));
        }
        let text = if is_raw(spec) {
            body
        } else {
            format!("Skills: {body}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::SkillsMetrics;

    fn ctx(pairs: &[(&str, u64)]) -> RenderContext {
        let mut m = SkillsMetrics::default();
        for (name, count) in pairs {
            m.skills_invoked.insert(name.to_string(), *count);
        }
        RenderContext {
            skills_metrics: Some(m),
            ..Default::default()
        }
    }

    #[test]
    fn empty_when_no_metrics() {
        let spans = SkillsWidget.render(&WidgetSpec::new("1", "skills"), &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_map_is_empty() {
        let spans = SkillsWidget.render(&WidgetSpec::new("1", "skills"), &ctx(&[]));
        assert!(spans.is_empty());
    }

    #[test]
    fn sorts_by_count_desc() {
        let spans = SkillsWidget.render(
            &WidgetSpec::new("1", "skills"),
            &ctx(&[("beta", 1), ("alpha", 3), ("gamma", 2)]),
        );
        assert_eq!(
            spans[0].text,
            "Skills: alpha\u{d7}3, gamma\u{d7}2, beta\u{d7}1"
        );
    }

    #[test]
    fn ties_break_alphabetically() {
        let spans =
            SkillsWidget.render(&WidgetSpec::new("1", "skills"), &ctx(&[("b", 2), ("a", 2)]));
        assert!(spans[0].text.starts_with("Skills: a\u{d7}2, b\u{d7}2"));
    }

    #[test]
    fn honors_limit_and_shows_overflow() {
        let mut spec = WidgetSpec::new("1", "skills");
        spec.metadata = Some(
            [("limit".to_string(), "2".to_string())]
                .into_iter()
                .collect(),
        );
        let spans = SkillsWidget.render(&spec, &ctx(&[("a", 5), ("b", 4), ("c", 3), ("d", 2)]));
        assert_eq!(spans[0].text, "Skills: a\u{d7}5, b\u{d7}4 +2");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "skills");
        spec.raw_value = Some(true);
        let spans = SkillsWidget.render(&spec, &ctx(&[("alpha", 1)]));
        assert_eq!(spans[0].text, "alpha\u{d7}1");
    }
}
