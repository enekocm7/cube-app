//! The right panel: current and best statistics of the session.

use cube_core::format::format_ms;
use cube_core::stats::Average;
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;

use crate::app::{CubeApp, average_label, format_average};

impl CubeApp {
    pub(crate) fn render_stats_panel(
        &self,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let strings = self.strings();
        let stats = &self.stats;
        let theme = cx.theme();
        let (border, muted) = (theme.border, theme.muted_foreground);
        let mono = theme.mono_font_family.clone();
        let optional_ms = |ms: Option<u64>| ms.map_or_else(|| "–".to_owned(), format_ms);

        let row = |label: String, current: String, best: String| {
            h_flex()
                .h(px(28.))
                .px_3()
                .gap_2()
                .text_sm()
                .child(div().w(px(52.)).text_color(muted).child(label))
                .child(
                    div()
                        .flex_1()
                        .font_family(mono.clone())
                        .text_right()
                        .child(current),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family(mono.clone())
                        .text_right()
                        .child(best),
                )
        };
        let summary = |label: &'static str, value: String| {
            h_flex()
                .h(px(26.))
                .px_3()
                .justify_between()
                .text_sm()
                .child(div().text_color(muted).child(label))
                .child(div().font_family(mono.clone()).child(value))
        };

        let latest = self
            .session()
            .solves()
            .last()
            .map_or_else(|| "–".to_owned(), ToString::to_string);
        let windows = stats.windows.iter().map(|window| {
            row(
                average_label(window.size),
                format_average(window.current),
                format_average(window.best.map(|(average, _): (Average, usize)| average)),
            )
        });

        v_flex()
            .w(width)
            .flex_shrink_0()
            .h_full()
            .py_2()
            .border_l_1()
            .border_color(border)
            .child(
                h_flex()
                    .h(px(28.))
                    .px_3()
                    .gap_2()
                    .text_xs()
                    .text_color(muted)
                    .child(div().w(px(52.)))
                    .child(div().flex_1().text_right().child(strings.stats_current))
                    .child(div().flex_1().text_right().child(strings.stats_best)),
            )
            .child(row(
                strings.stat_single.to_owned(),
                latest,
                optional_ms(stats.best.map(|(ms, _)| ms)),
            ))
            .children(windows)
            .child(div().my_2().mx_3().h(px(1.)).bg(border))
            .child(summary(strings.stat_mean, optional_ms(stats.mean)))
            .child(summary(strings.stat_std_dev, optional_ms(stats.std_dev)))
            .child(summary(
                strings.stat_worst,
                optional_ms(stats.worst.map(|(ms, _)| ms)),
            ))
            .child(summary(strings.stat_solves, stats.count.to_string()))
            .child(summary(strings.stat_dnfs, stats.dnf_count.to_string()))
    }
}
