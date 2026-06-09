use std::sync::Arc;

use eframe::egui;

pub(crate) fn install_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut ui_font_name = None;
    if let Some(font_bytes) = load_first_existing_font(cjk_font_candidates()) {
        let font_name = "stcjudge-cjk-ui".to_owned();
        fonts.font_data.insert(
            font_name.clone(),
            Arc::new(egui::FontData::from_owned(font_bytes)),
        );
        prepend_font(&mut fonts, egui::FontFamily::Proportional, &font_name);
        ui_font_name = Some(font_name);
    }
    if let Some(font_bytes) = load_first_existing_font(cjk_monospace_font_candidates()) {
        let font_name = "stcjudge-cjk-mono".to_owned();
        fonts.font_data.insert(
            font_name.clone(),
            Arc::new(egui::FontData::from_owned(font_bytes)),
        );
        prepend_font(&mut fonts, egui::FontFamily::Monospace, &font_name);
    } else if let Some(font_name) = ui_font_name {
        prepend_font(&mut fonts, egui::FontFamily::Monospace, &font_name);
    }
    ctx.set_fonts(fonts);
}

pub(crate) fn tune_ui_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    set_text_size(&mut style, egui::TextStyle::Small, 11.0);
    set_text_size(&mut style, egui::TextStyle::Body, 13.0);
    set_text_size(&mut style, egui::TextStyle::Button, 13.0);
    set_text_size(&mut style, egui::TextStyle::Monospace, 13.0);
    set_text_size(&mut style, egui::TextStyle::Heading, 17.0);
    ctx.set_global_style(style);
}

fn prepend_font(fonts: &mut egui::FontDefinitions, family: egui::FontFamily, font_name: &str) {
    let family_fonts = fonts.families.entry(family).or_default();
    family_fonts.retain(|existing| existing != font_name);
    family_fonts.insert(0, font_name.to_owned());
}

fn set_text_size(style: &mut egui::Style, text_style: egui::TextStyle, size: f32) {
    if let Some(font_id) = style.text_styles.get_mut(&text_style) {
        font_id.size = size;
    }
}

fn load_first_existing_font(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| std::fs::read(path).ok())
}

fn cjk_font_candidates() -> &'static [&'static str] {
    &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
    ]
}

fn cjk_monospace_font_candidates() -> &'static [&'static str] {
    &[
        "/System/Library/Fonts/Supplemental/Noto Sans Mono CJK SC.otf",
        "/System/Library/Fonts/Supplemental/Noto Sans Mono CJK TC.otf",
        "/Library/Fonts/Noto Sans Mono CJK SC.otf",
        "/Library/Fonts/Noto Sans Mono CJK TC.otf",
        "/Library/Fonts/Sarasa Mono SC Nerd Font.ttf",
        "/Library/Fonts/Sarasa Mono SC.ttf",
        "/Library/Fonts/LXGW WenKai Mono.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansMonoCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansMonoCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
    ]
}
