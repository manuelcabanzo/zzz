use eframe::egui;

pub fn context_menu<F: FnOnce(&mut egui::Ui)>(
    ui: &mut egui::Ui,
    content: F,
    menu_id: &mut Option<egui::Id>,
    is_menu_open: &mut bool,
) {
    ui.input(|input| {
        if input.pointer.secondary_clicked() {
            *is_menu_open = true;
            *menu_id = Some(ui.id());
        }
    });

    if *is_menu_open {
        let popup_response = egui::Area::new(menu_id.unwrap())
            .order(egui::Order::Foreground)
            .fixed_pos(ui.ctx().pointer_interact_pos().unwrap_or_default())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        content(ui);
                    });
            });

        if popup_response.response.clicked_elsewhere() {
            *is_menu_open = false;
        }
    }
}
