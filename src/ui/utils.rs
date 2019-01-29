use imgui::{ImColor, ImGuiCol, ImStr, ImVec2, ImVec4, Ui};

pub fn text_bg_color(ui: &Ui) -> ImVec4 {
    ui.imgui().style().colors[ImGuiCol::Button as usize]
}

pub fn text_with_bg<P, S, C>(ui: &Ui, pos: P, s: S, color: Option<C>)
where
    P: Into<ImVec2>,
    S: AsRef<ImStr>,
    C: Into<ImColor>,
{
    let ds = ui.calc_text_size(s.as_ref(), false, 0.0);
    let pos = pos.into();

    if let Some(c) = color {
        let (wx, wy) = ui.get_window_pos();

        ui.get_window_draw_list()
            .add_rect(
                [wx + pos.x - ds.x * 0.5, wy + pos.y - ds.y * 0.2],
                [wx + pos.x + ds.x * 1.5, wy + pos.y + ds.y * 1.2],
                c,
            )
            .filled(true)
            .build();
    }

    let (ox, oy) = ui.get_cursor_pos();
    ui.set_cursor_pos(pos);
    ui.text_wrapped(s.as_ref());
    let (nx, ny) = ui.get_cursor_pos();

    ui.set_cursor_pos((ox + (nx - pos.x), oy + (ny - pos.y)));
}
