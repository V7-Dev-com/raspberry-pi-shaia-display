use shaia_display::text::Schriften;
fn main() {
    let mut sf = Schriften::neu(std::env::var("PI_FONTS").unwrap_or("../schriften".into()));
    for g in [22, 20, 18, 16] {
        println!("{g}: {}", sf.breite("GEWITTER +2", "Rajdhani-SemiBold.ttf", g, 0));
    }
}
