fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(9876);
    speak_up_backend::run_with_port(port);
}
