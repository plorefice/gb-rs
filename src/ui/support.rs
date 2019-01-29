use conrod_core::Theme;

pub fn theme() -> Theme {
    use conrod_core::{
        color,
        position::{Align, Direction, Padding, Position, Relative},
        theme::StyleMap,
    };

    Theme {
        name: "gb-rs theme".to_string(),
        padding: Padding::none(),
        x_position: Position::Relative(Relative::Align(Align::Start), None),
        y_position: Position::Relative(Relative::Direction(Direction::Backwards, 20.0), None),
        background_color: color::DARK_CHARCOAL,
        shape_color: color::LIGHT_CHARCOAL,
        border_color: color::BLACK,
        border_width: 1.0,
        label_color: color::WHITE,
        font_id: None,
        font_size_large: 13,
        font_size_medium: 11,
        font_size_small: 9,
        widget_styling: StyleMap::default(),
        mouse_drag_threshold: 0.0,
        double_click_threshold: std::time::Duration::from_millis(500),
    }
}
