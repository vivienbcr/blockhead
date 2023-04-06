#[cfg(test)]
pub fn setup() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter(Some("blockhead"), log::LevelFilter::Trace)
        .try_init();
}
