const MAX_TASK_LEN: usize = 128;

/// Validate a session task name before using it in filesystem paths or URLs.
pub fn validate_task_name(task: &str) -> Result<&str, String> {
    let task = task.trim();
    if task.is_empty() {
        return Err("task is required".into());
    }
    if task.len() > MAX_TASK_LEN {
        return Err(format!("task name too long (max {MAX_TASK_LEN})"));
    }
    if task == "." || task == ".." {
        return Err("invalid task name".into());
    }
    if task.contains('/') || task.contains('\\') || task.contains('\0') {
        return Err("task name must not contain path separators".into());
    }
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_names() {
        assert_eq!(validate_task_name("demo").unwrap(), "demo");
        assert_eq!(validate_task_name("  my-task_1  ").unwrap(), "my-task_1");
    }

    #[test]
    fn rejects_traversal_and_separators() {
        assert!(validate_task_name("").is_err());
        assert!(validate_task_name("..").is_err());
        assert!(validate_task_name(".").is_err());
        assert!(validate_task_name("../etc").is_err());
        assert!(validate_task_name("a/b").is_err());
        assert!(validate_task_name("a\\b").is_err());
    }
}
