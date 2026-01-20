/// Color theory utilities for generating harmonious color schemes
use eframe::egui;

/// Convert RGB to HSV color space
/// Returns (hue [0-360], saturation [0-1], value [0-1])
fn rgb_to_hsv(color: egui::Color32) -> (f32, f32, f32) {
    let r = color.r() as f32 / 255.0;
    let g = color.g() as f32 / 255.0;
    let b = color.b() as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let hue = if hue < 0.0 { hue + 360.0 } else { hue };

    let saturation = if max == 0.0 { 0.0 } else { delta / max };

    (hue, saturation, max)
}

/// Convert HSV to RGB color space
fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> egui::Color32 {
    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;

    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    egui::Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Rotate hue by degrees, keeping saturation and value
fn rotate_hue(color: egui::Color32, degrees: f32) -> egui::Color32 {
    let (mut hue, sat, val) = rgb_to_hsv(color);
    hue = (hue + degrees) % 360.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    hsv_to_rgb(hue, sat, val)
}

/// Adjust saturation by a factor (0.0 to 1.0 reduces, >1.0 increases)
fn adjust_saturation(color: egui::Color32, factor: f32) -> egui::Color32 {
    let (hue, sat, val) = rgb_to_hsv(color);
    let new_sat = (sat * factor).clamp(0.0, 1.0);
    hsv_to_rgb(hue, new_sat, val)
}

/// Adjust value/brightness by a factor
fn adjust_value(color: egui::Color32, factor: f32) -> egui::Color32 {
    let (hue, sat, val) = rgb_to_hsv(color);
    let new_val = (val * factor).clamp(0.0, 1.0);
    hsv_to_rgb(hue, sat, new_val)
}

/// Generate a 4-color scheme from a primary color using complementary + analogous
pub struct ColorScheme {
    pub primary: egui::Color32,
    pub secondary: egui::Color32,
    pub tertiary: egui::Color32,
    pub quaternary: egui::Color32,
}

impl ColorScheme {
    /// Generate a monochromatic scheme with varying brightness/saturation
    /// Like the examples: all same hue, progressively darker/desaturated for supporting colors
    pub fn from_primary(primary: egui::Color32) -> Self {
        // All colors share the same hue as primary, but vary in brightness and saturation
        // Primary: Full brightness (user's chosen color)
        // Secondary: Medium-dark, 60% brightness, 70% saturation
        let secondary = adjust_saturation(adjust_value(primary, 0.6), 0.7);

        // Tertiary: Darker, 40% brightness, 50% saturation
        let tertiary = adjust_saturation(adjust_value(primary, 0.4), 0.5);

        // Quaternary: Very dark, 25% brightness, 30% saturation (for backgrounds)
        let quaternary = adjust_saturation(adjust_value(primary, 0.25), 0.3);

        Self {
            primary,
            secondary,
            tertiary,
            quaternary,
        }
    }

    /// Generate a dark theme Visuals from this color scheme
    pub fn to_dark_visuals(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::dark();

        // Window backgrounds - use quaternary (darkest)
        visuals.extreme_bg_color = adjust_value(self.quaternary, 0.5); // Even darker for extreme bg
        visuals.window_fill = self.quaternary; // Darkest color for main background
        visuals.panel_fill = adjust_value(self.quaternary, 1.2); // Slightly lighter for panels

        // Widgets - monochromatic progression
        // Inactive state: use tertiary (dark)
        visuals.widgets.inactive.bg_fill = self.tertiary;
        visuals.widgets.inactive.weak_bg_fill = adjust_value(self.tertiary, 0.8);
        visuals.widgets.inactive.bg_stroke.color = adjust_value(self.tertiary, 1.2);

        // Hovered state: use secondary (medium)
        visuals.widgets.hovered.bg_fill = self.secondary;
        visuals.widgets.hovered.weak_bg_fill = adjust_value(self.secondary, 0.8);
        visuals.widgets.hovered.bg_stroke.color = adjust_value(self.secondary, 1.2);

        // Active state: use primary (brightest)
        visuals.widgets.active.bg_fill = self.primary;
        visuals.widgets.active.weak_bg_fill = adjust_value(self.primary, 0.85);
        visuals.widgets.active.bg_stroke.color = adjust_value(self.primary, 1.1);

        // Open state (for collapsing headers, etc): use tertiary
        visuals.widgets.open.bg_fill = self.tertiary;
        visuals.widgets.open.weak_bg_fill = adjust_value(self.tertiary, 0.8);
        visuals.widgets.open.bg_stroke.color = self.secondary;

        // Selection color: use secondary (accent color)
        visuals.selection.bg_fill = self.secondary;
        visuals.selection.stroke.color = self.primary;

        // Hyperlinks: use primary for vibrancy
        visuals.hyperlink_color = self.primary;

        // Window stroke/border: use secondary
        visuals.window_stroke.color = self.secondary;

        // Code background: slightly lighter than window
        visuals.code_bg_color = adjust_value(self.quaternary, 1.3);

        // Faint background: between extreme and window
        visuals.faint_bg_color = adjust_value(self.quaternary, 0.7);

        // Window rounding and shadows to match theme
        visuals.window_rounding = egui::Rounding::same(6.0);
        visuals.window_shadow.color = adjust_value(self.quaternary, 0.3);

        visuals
    }

    /// Generate a light theme Visuals from this color scheme
    pub fn to_light_visuals(&self) -> egui::Visuals {
        let mut visuals = egui::Visuals::light();

        // Window backgrounds
        visuals.window_fill = egui::Color32::from_gray(245);
        visuals.panel_fill = egui::Color32::from_gray(240);
        visuals.extreme_bg_color = egui::Color32::WHITE;

        // Widgets - use our color scheme with lighter values
        // Inactive state: primary color, light
        visuals.widgets.inactive.bg_fill = adjust_saturation(adjust_value(self.primary, 0.9), 0.3);
        visuals.widgets.inactive.weak_bg_fill = adjust_saturation(adjust_value(self.primary, 0.95), 0.2);
        visuals.widgets.inactive.bg_stroke.color = adjust_value(self.primary, 0.7);

        // Hovered state: secondary color
        visuals.widgets.hovered.bg_fill = adjust_saturation(adjust_value(self.secondary, 0.85), 0.4);
        visuals.widgets.hovered.weak_bg_fill = adjust_saturation(adjust_value(self.secondary, 0.92), 0.3);
        visuals.widgets.hovered.bg_stroke.color = adjust_value(self.secondary, 0.6);

        // Active state: primary color, saturated
        visuals.widgets.active.bg_fill = adjust_value(self.primary, 0.7);
        visuals.widgets.active.weak_bg_fill = adjust_value(self.primary, 0.8);
        visuals.widgets.active.bg_stroke.color = self.primary;

        // Open state: tertiary
        visuals.widgets.open.bg_fill = adjust_saturation(adjust_value(self.tertiary, 0.88), 0.35);
        visuals.widgets.open.weak_bg_fill = adjust_value(self.tertiary, 0.93);
        visuals.widgets.open.bg_stroke.color = adjust_value(self.tertiary, 0.7);

        // Selection color: quaternary
        visuals.selection.bg_fill = adjust_saturation(adjust_value(self.quaternary, 0.8), 0.5);
        visuals.selection.stroke.color = adjust_value(self.quaternary, 0.6);

        // Hyperlinks: secondary color, darker
        visuals.hyperlink_color = adjust_value(self.secondary, 0.5);

        // Window stroke/border: primary
        visuals.window_stroke.color = adjust_value(self.primary, 0.6);

        visuals
    }
}

/// Default color schemes for presets
impl Default for ColorScheme {
    fn default() -> Self {
        // Nice blue as default
        Self::from_primary(egui::Color32::from_rgb(64, 128, 255))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_hsv_conversion() {
        let color = egui::Color32::from_rgb(255, 0, 0); // Red
        let (h, s, v) = rgb_to_hsv(color);
        assert!((h - 0.0).abs() < 1.0);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        let converted = hsv_to_rgb(h, s, v);
        assert_eq!(color, converted);
    }

    #[test]
    fn test_hue_rotation() {
        let red = egui::Color32::from_rgb(255, 0, 0);
        let cyan = rotate_hue(red, 180.0);
        // Cyan should be close to RGB(0, 255, 255)
        assert!(cyan.b() > 200);
        assert!(cyan.g() > 200);
        assert!(cyan.r() < 50);
    }
}
