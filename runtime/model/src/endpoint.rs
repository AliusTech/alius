pub(crate) fn normalize_openai_api_base(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    trimmed
        .strip_suffix("/chat/completions")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_string()
}

pub(crate) fn normalize_anthropic_api_base(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    trimmed
        .strip_suffix("/v1/messages")
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_endpoint_url_normalizes_to_api_base() {
        assert_eq!(
            normalize_openai_api_base("https://api.xiaomimimo.com/v1/chat/completions"),
            "https://api.xiaomimimo.com/v1"
        );
    }

    #[test]
    fn anthropic_messages_url_normalizes_to_api_base() {
        assert_eq!(
            normalize_anthropic_api_base("https://api.xiaomimimo.com/anthropic/v1/messages"),
            "https://api.xiaomimimo.com/anthropic"
        );
    }
}
