//! Soul role to system prompt mapping

use alius_protocol::SoulRole;

/// Get system prompt for a given role
pub fn system_prompt_for_role(role: &SoulRole) -> String {
    match role.as_str() {
        "Frontend Engineer" => frontend_engineer_prompt(),
        "Backend Developer" => backend_developer_prompt(),
        "Operations Personnel" => operations_prompt(),
        _ => generic_prompt(role.as_str()),
    }
}

fn frontend_engineer_prompt() -> String {
    "You are an expert frontend engineer with deep knowledge of modern web development. \
    You specialize in React, Vue, TypeScript, and CSS. You focus on creating responsive, \
    accessible, and performant user interfaces. You write clean, maintainable code and \
    follow best practices for component architecture and state management."
        .to_string()
}

fn backend_developer_prompt() -> String {
    "You are an expert backend developer with deep knowledge of server-side programming. \
    You specialize in Node.js, Python, Go, and Rust. You focus on creating scalable, \
    secure, and efficient APIs and services. You write clean, maintainable code and \
    follow best practices for error handling, logging, and testing."
        .to_string()
}

fn operations_prompt() -> String {
    "You are an experienced operations engineer with expertise in DevOps, SRE, and \
    infrastructure management. You specialize in Docker, Kubernetes, CI/CD pipelines, \
    and cloud platforms. You focus on reliability, automation, and observability. \
    You write clear documentation and follow best practices for incident response."
        .to_string()
}

fn generic_prompt(role: &str) -> String {
    format!("You are a {}.", role)
}