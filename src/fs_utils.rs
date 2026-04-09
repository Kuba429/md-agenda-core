use std::fs::{self, File};
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub fn file_lines_get(filename: &str) -> Vec<String> {
    match file_lines_get_result(filename) {
        Ok(lines) => lines,
        Err(_) => vec![],
    }
}

pub fn file_lines_get_result(filename: &str) -> Result<Vec<String>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().filter_map(Result::ok).collect();
    Ok(lines)
}

fn bullet_indent_level(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

pub fn bullet_parent_get(lines: &Vec<String>, line_number: usize) -> Option<usize> {
    if line_number == 0 || line_number > lines.len() {
        return None;
    }

    let current_line_index = line_number - 1;
    let current_indent = bullet_indent_level(&lines[current_line_index]);

    for i in (0..current_line_index).rev() {
        let candidate_line = &lines[i];
        let candidate_indent = bullet_indent_level(candidate_line);

        if candidate_indent < current_indent {
            let trimmed = candidate_line.trim_start();
            if trimmed.starts_with("* ") || trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
                return Some(i + 1);
            }
        }
    }

    None
}

pub fn file_line_insert(
    filename: &str,
    line_number: Option<usize>,
    content: &str,
) -> std::io::Result<()> {
    ensure_file_exists(filename)?;

    let mut lines = file_lines_get(filename);

    match line_number {
        Some(idx) if idx > 0 && idx <= lines.len() => lines.insert(idx - 1, content.to_string()),
        _ => lines.push(content.to_string()), // append to end if None or invalid
    }

    let mut file = File::create(filename)?;
    for line in lines {
        writeln!(file, "{}", line)?;
    }

    Ok(())
}

pub fn ensure_file_exists(filename: &str) -> Result<()> {
    let path = Path::new(filename);

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    if !path.exists() {
        File::create(path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_bullet_indent_level() {
        assert_eq!(bullet_indent_level("  - item"), 2);
        assert_eq!(bullet_indent_level("    * item"), 4);
        assert_eq!(bullet_indent_level("- item"), 0);
        assert_eq!(bullet_indent_level("no bullet"), 0);
    }

    #[test]
    fn test_bullet_parent_get_no_parent() {
        let lines = vec!["* First".to_string(), "* Second".to_string()];

        // Same-level siblings don't match (indent not less)
        assert_eq!(bullet_parent_get(&lines, 2), None);
    }

    #[test]
    fn test_file_line_insert_append() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"line 1\nline 2\n").unwrap();

        let path = temp_file.path().to_str().unwrap();
        file_line_insert(path, None, "line 3").unwrap();

        let lines = file_lines_get(path);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2], "line 3");
    }

    #[test]
    fn test_file_line_insert_at_position() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"line 1\nline 2\nline 3\n").unwrap();

        let path = temp_file.path().to_str().unwrap();
        file_line_insert(path, Some(2), "inserted").unwrap();

        let lines = file_lines_get(path);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[1], "inserted");
    }

    #[test]
    fn test_ensure_file_exists_creates_file() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_ensure_file.txt");
        let path_str = temp_path.to_str().unwrap();

        if temp_path.exists() {
            std::fs::remove_file(&temp_path).unwrap();
        }

        ensure_file_exists(path_str).unwrap();
        assert!(temp_path.exists());

        std::fs::remove_file(&temp_path).ok();
    }

    #[test]
    fn test_ensure_file_exists_creates_parent_dirs() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("nested").join("test").join("file.txt");
        let path_str = temp_path.to_str().unwrap();

        if temp_path.exists() {
            std::fs::remove_file(&temp_path).ok();
        }
        if temp_path.parent().map_or(false, |p| p.exists()) {
            std::fs::remove_dir_all(temp_path.parent().unwrap()).ok();
        }

        ensure_file_exists(path_str).unwrap();
        assert!(temp_path.exists());

        temp_path.parent().map(|p| std::fs::remove_dir_all(p).ok());
    }
}
