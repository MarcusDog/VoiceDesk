fn main() {
    ensure_windows_icon();
    tauri_build::build()
}

fn ensure_windows_icon() {
    let icon_path = std::path::Path::new("icons/icon.ico");
    if icon_path.exists() {
        return;
    }
    std::fs::create_dir_all("icons").expect("create icon directory");

    const WIDTH: usize = 32;
    const HEIGHT: usize = 32;
    let pixel_bytes = WIDTH * HEIGHT * 4;
    let mask_bytes = 4 * HEIGHT;
    let image_bytes = 40 + pixel_bytes + mask_bytes;
    let mut icon = Vec::with_capacity(22 + image_bytes);

    icon.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
    icon.extend_from_slice(&[WIDTH as u8, HEIGHT as u8, 0, 0]);
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&(image_bytes as u32).to_le_bytes());
    icon.extend_from_slice(&22u32.to_le_bytes());

    icon.extend_from_slice(&40u32.to_le_bytes());
    icon.extend_from_slice(&(WIDTH as i32).to_le_bytes());
    icon.extend_from_slice(&((HEIGHT * 2) as i32).to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());

    let bars = [(8usize, 10usize), (12, 17), (16, 22), (20, 15), (24, 9)];
    for y in (0..HEIGHT).rev() {
        for x in 0..WIDTH {
            let dx = if x < 6 { 6 - x } else { x.saturating_sub(25) };
            let dy = if y < 6 { 6 - y } else { y.saturating_sub(25) };
            let inside = (3..=28).contains(&x) && (3..=28).contains(&y) && dx * dx + dy * dy <= 10;
            let is_bar = bars.iter().any(|(bar_x, height)| {
                x >= *bar_x
                    && x <= *bar_x + 1
                    && y >= (HEIGHT - height) / 2
                    && y < (HEIGHT + height) / 2
            });
            if inside && is_bar {
                icon.extend_from_slice(&[255, 255, 255, 255]);
            } else if inside {
                icon.extend_from_slice(&[238, 111, 20, 255]);
            } else {
                icon.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    icon.resize(22 + image_bytes, 0);
    std::fs::write(icon_path, icon).expect("write generated Windows icon");
}
