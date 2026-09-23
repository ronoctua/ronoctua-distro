//! `ronoctua-keys` — the daemon of Independent mode. See the library.

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    // THE SESSION'S USER, from the unit: the socket is theirs and nobody
    // else's, because diverted keys cross it.
    let user = std::env::var("RONOCTUA_KEYS_USER").ok();
    let directory = std::env::var("RONOCTUA_KEYS_DIRECTORY")
        .unwrap_or_else(|_| ronoctua_keys::config::DIRECTORY.to_owned());
    ronoctua_keys::daemon::run(&directory, user.as_deref())
}
