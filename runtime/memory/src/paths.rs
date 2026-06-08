use std::path::PathBuf;

pub(crate) fn global_alius_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".alius")
}

pub(crate) fn project_alius_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from);
    let mut dir = cwd.as_path();

    loop {
        if home.as_deref() == Some(dir) {
            return cwd.join(".alius");
        }

        let candidate = dir.join(".alius");
        if candidate.exists() {
            return candidate;
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => return cwd.join(".alius"),
        }
    }
}

pub(crate) fn project_communication_sessions_dir() -> PathBuf {
    project_memory_dir().join("communications").join("sessions")
}

pub(crate) fn project_memory_dir() -> PathBuf {
    project_alius_dir().join("memory")
}
