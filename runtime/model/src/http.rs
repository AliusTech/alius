pub(crate) fn user_agent() -> String {
    let version = option_env!("ALIUS_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    format!("alius-cli/{version}")
}
