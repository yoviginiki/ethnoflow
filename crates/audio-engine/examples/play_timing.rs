use audio_engine::{AudioBuffer, Player};
fn main() {
    let tone = AudioBuffer::sine(44_100, 440.0, 3.0, 0.05);
    let t0 = std::time::Instant::now();
    let p = Player::start(&tone, None, 0).expect("player");
    while !p.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    println!(
        "3.0 s buffer finished in {:.2} s wall time, status {:?}",
        t0.elapsed().as_secs_f32(),
        p.status()
    );
}
