use std::fs::{self, File};
use std::io::Result;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

pub fn file_lines_get(filename: &str) -> Vec<String> {
    let file = File::open(filename).expect("can't open file");
    let reader = BufReader::new(file);
    reader.lines().filter_map(Result::ok).collect()
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
