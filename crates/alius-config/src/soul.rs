//! Soul role to system prompt mapping.
//!
//! This module maps soul roles (agent personas) to their corresponding
//! system prompts. Each role defines the agent's expertise, behavior,
//! and response style.

use alius_protocol::SoulRole;

/// Get the system prompt for a given soul role.
///
/// Maps role names to detailed system prompts that define the agent's
/// expertise and behavior. Falls back to a generic prompt for unknown roles.
///
/// # Arguments
/// * `role` - The soul role to get the system prompt for.
///
/// # Returns
/// A system prompt string that should be sent to the LLM as a system message.
pub fn system_prompt_for_role(role: &SoulRole) -> String {
    match role.as_str() {
        "Frontend Engineer" => frontend_engineer_prompt(),
        "Backend Developer" => backend_developer_prompt(),
        "Operations Personnel" => operations_prompt(),
        _ => generic_prompt(role.as_str()),
    }
}

/// System prompt for the "Frontend Engineer" role.
///
/// Specializes in modern web development with React, Vue, TypeScript, and CSS.
/// Focuses on responsive, accessible, and performant user interfaces.
fn frontend_engineer_prompt() -> String {
    "You are an expert frontend engineer with deep knowledge of modern web development. \
    You specialize in React, Vue, TypeScript, and CSS. You focus on creating responsive, \
    accessible, and performant user interfaces. You write clean, maintainable code and \
    follow best practices for component architecture and state management."
        .to_string()
}

/// System prompt for the "Backend Developer" role.
///
/// Specializes in server-side programming with Node.js, Python, Go, and Rust.
/// Focuses on scalable, secure, and efficient APIs and services.
fn backend_developer_prompt() -> String {
    "You are an expert backend developer with deep knowledge of server-side programming. \
    You specialize in Node.js, Python, Go, and Rust. You focus on creating scalable, \
    secure, and efficient APIs and services. You write clean, maintainable code and \
    follow best practices for error handling, logging, and testing."
        .to_string()
}

/// System prompt for the "Operations Personnel" role.
///
/// Specializes in DevOps, SRE, and infrastructure management.
/// Focuses on reliability, automation, and observability.
fn operations_prompt() -> String {
    "You are an experienced operations engineer with expertise in DevOps, SRE, and \
    infrastructure management. You specialize in Docker, Kubernetes, CI/CD pipelines, \
    and cloud platforms. You focus on reliability, automation, and observability. \
    You write clear documentation and follow best practices for incident response."
        .to_string()
}

/// Generic system prompt for unknown roles.
///
/// Creates a simple prompt with the role name as the expertise area.
fn generic_prompt(role: &str) -> String {
    format!("You are a {}.", role)
}
