use rayon::prelude::*;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::config::AgendaConfig;

pub static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());

pub static BLOCK_START_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)[-*] ").unwrap());

pub static BACKLINKS_SECTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!-- BACKLINKS:START -->.*?<!-- BACKLINKS:END -->").unwrap());

#[derive(Debug, Clone)]
pub struct Link {
    pub target: String,
}

impl Link {
    pub fn from_capture(capture: &str) -> Self {
        Link {
            target: capture.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LinkIndex {
    links_to_sources: std::collections::HashMap<String, Vec<String>>,
    source_to_links: std::collections::HashMap<String, Vec<String>>,
}

impl LinkIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(files: &[(String, std::path::PathBuf)]) -> Self {
        let mut index = Self::new();

        for (filename, file_path) in files {
            if !file_path.exists() {
                continue;
            }

            let content = match std::fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let clean_content = skip_backlink_section(&content);
            let blocks = extract_blocks(&clean_content, filename);

            let mut links_in_file = Vec::new();

            for block in blocks {
                for link in block.extract_links() {
                    index
                        .links_to_sources
                        .entry(link.target.clone())
                        .or_insert_with(Vec::new)
                        .push(filename.clone());

                    links_in_file.push(link.target);
                }
            }

            index
                .source_to_links
                .insert(filename.clone(), links_in_file);
        }

        index
    }

    pub fn get_sources_for_link(&self, target: &str) -> Vec<&str> {
        self.links_to_sources
            .get(target)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn get_sources_for_link_with_duplicates(&self, target: &str) -> Vec<&str> {
        self.links_to_sources
            .get(target)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    pub fn get_links_in_file(&self, filename: &str) -> Vec<&str> {
        self.source_to_links
            .get(filename)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub source_file: String,
    pub indent_level: usize,
    pub lines: Vec<String>,
}

impl Block {
    pub fn new(source_file: &str, indent_level: usize, lines: Vec<String>) -> Self {
        Block {
            source_file: source_file.to_string(),
            indent_level,
            lines,
        }
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn extract_links(&self) -> Vec<Link> {
        let content = self.content();
        LINK_RE
            .captures_iter(&content)
            .filter_map(|cap| cap.get(1))
            .map(|m| Link::from_capture(m.as_str()))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Backlink {
    pub source_file: String,
    pub blocks: Vec<Block>,
}

impl Backlink {
    pub fn new(source_file: &str) -> Self {
        Backlink {
            source_file: source_file.to_string(),
            blocks: Vec::new(),
        }
    }

    pub fn add_block(&mut self, block: Block) {
        self.blocks.push(block);
    }
}

pub fn scan_vault_files(vault_path: &Path) -> Vec<(String, std::path::PathBuf)> {
    let mut files = Vec::new();
    scan_dir_recursive(vault_path, vault_path, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn scan_dir_recursive(
    dir: &Path,
    vault_path: &Path,
    files: &mut Vec<(String, std::path::PathBuf)>,
) {
    if let Ok(entries) = dir.read_dir() {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                scan_dir_recursive(&path, vault_path, files);
            } else if path.extension().map_or(false, |ext| ext == "md") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    files.push((stem.to_string(), path));
                }
            }
        }
    }
}

pub fn read_file_content(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read file {:?}: {}", path, e))
}

pub fn write_file_content(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| format!("Failed to write file {:?}: {}", path, e))
}

pub fn remove_existing_section(content: &str) -> String {
    let removed = BACKLINKS_SECTION_RE.replace_all(content, "").to_string();
    let trimmed = removed.trim();
    if trimmed.is_empty() {
        String::new()
    } else {
        trimmed.to_string()
    }
}

#[derive(Clone)]
struct TreeNode {
    text: String,
    children: Vec<TreeNode>,
}

fn is_link_only_line(text: &str) -> bool {
    if let Some(pos) = text.find("]]") {
        text[pos + 2..].trim().is_empty()
    } else {
        false
    }
}

fn extract_link_subtree(block: &Block, link_pos: usize) -> Vec<TreeNode> {
    if link_pos >= block.lines.len() {
        return Vec::new();
    }

    let link_line = &block.lines[link_pos];
    let link_indent = link_line.len() - link_line.trim_start().len();
    let link_text = link_line.trim().to_string();

    let root = TreeNode {
        text: link_text,
        children: Vec::new(),
    };

    vec![build_tree_recursive(block, link_pos, link_indent, root)]
}

fn build_tree_recursive(
    block: &Block,
    parent_pos: usize,
    parent_indent: usize,
    mut node: TreeNode,
) -> TreeNode {
    let mut i = parent_pos + 1;
    while i < block.lines.len() {
        let line = &block.lines[i];
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if indent <= parent_indent {
            break;
        }

        let child = build_tree_recursive(
            block,
            i,
            indent,
            TreeNode {
                text: trimmed.to_string(),
                children: Vec::new(),
            },
        );

        node.children.push(child);
        i += 1;
    }

    node
}

fn render_tree(nodes: &[TreeNode], link_indent: usize, base_indent: usize, output: &mut String) {
    for node in nodes {
        if is_link_only_line(&node.text) {
            render_tree(&node.children, link_indent, base_indent, output);
            continue;
        }

        let indent = (base_indent + link_indent).min(40);
        output.push_str(&" ".repeat(indent));
        output.push_str(&node.text);
        output.push('\n');

        if !node.children.is_empty() {
            render_tree(&node.children, link_indent, base_indent, output);
        }
    }
}

pub fn build_backlink_section(backlinks: &[Backlink]) -> String {
    if backlinks.is_empty() {
        return String::new();
    }

    let mut section = String::new();
    section.push_str("\n<!-- BACKLINKS:START -->\n## Backlinks\n\n");

    for (i, backlink) in backlinks.iter().enumerate() {
        if i > 0 {
            section.push('\n');
        }

        if backlink.blocks.is_empty() {
            section.push_str("- [[");
            section.push_str(&backlink.source_file);
            section.push_str("]]");
            continue;
        }

        for (block_idx, block) in backlink.blocks.iter().enumerate() {
            if block_idx > 0 {
                section.push_str("\n");
            }

            let first_line = block.lines.first().map(|s| s.as_str()).unwrap_or("");
            let first_line_has_multi_links = first_line.matches("[[").count() > 1;
            let first_line_indent = first_line.len() - first_line.trim_start().len();
            let link_line_indent;

            if first_line_has_multi_links {
                let multi_link_indent = first_line_indent.saturating_add(4).min(40);
                section.push_str("- [[");
                section.push_str(&backlink.source_file);
                section.push_str("]]");
                section.push_str("\n");
                section.push_str(&" ".repeat(multi_link_indent));
                section.push_str(first_line.trim());
                link_line_indent = multi_link_indent;
            } else {
                let link_pos = block.lines.iter().position(|l| {
                    let t = l.trim();
                    (t.starts_with('*') || t.starts_with('-')) && t.contains("[[")
                });

                if let Some(pos) = link_pos {
                    if pos == 0 {
                        section.push_str("- [[");
                        section.push_str(&backlink.source_file);
                        section.push_str("]]");
                        link_line_indent = first_line_indent;
                    } else {
                        section.push_str("- [[");
                        section.push_str(&backlink.source_file);
                        section.push_str("]]");
                        link_line_indent =
                            block.lines[pos].len() - block.lines[pos].trim_start().len();

                        let subtree = extract_link_subtree(block, pos);
                        if !subtree.is_empty() {
                            section.push('\n');
                            render_tree(&subtree, link_line_indent, 2, &mut section);
                        }
                        continue;
                    }
                } else {
                    section.push_str("- [[");
                    section.push_str(&backlink.source_file);
                    section.push_str("]]");
                    link_line_indent = first_line_indent;
                }
            }

            if block.lines.len() <= 1 {
                continue;
            }

            let base_indent_for_children = if first_line_has_multi_links {
                first_line_indent
            } else {
                link_line_indent
            };
            section.push('\n');

            static LINK_ONLY_RE: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"^\s+\*\s*\[\[[^\]]+\]\]\s*$").unwrap());

            let mut last_was_empty = false;
            let mut empty_line_count = 0;

            let first_link_pos = block.lines.iter().position(|l| l.contains("[["));

            for (j, line) in block.lines.iter().skip(1).enumerate() {
                if let Some(first_link) = first_link_pos {
                    if first_link > 0 && j < first_link {
                        let has_link_later = block.lines[j..].iter().any(|l| l.contains("[["));
                        if !has_link_later {
                            continue;
                        }
                    }
                }

                let _is_last_line = j == block.lines.len() - 2;
                let trimmed = line.trim();
                let current_indent = line.len() - line.trim_start().len();
                let line_indent = current_indent;

                let is_whitespace_only = trimmed.is_empty() && line.len() > line.trim_start().len();

                if is_whitespace_only {
                    continue;
                }

                if trimmed.is_empty() {
                    if !last_was_empty {
                        empty_line_count = 1;
                    } else {
                        empty_line_count += 1;
                    }
                    last_was_empty = true;
                    continue;
                }

                let is_bullet_line =
                    line.trim_start().starts_with('*') || line.trim_start().starts_with('-');

                let should_break = line_indent <= link_line_indent && is_bullet_line;

                if is_bullet_line && !line.contains("[[") && j > 0 {
                    let mut prev_link_indent = link_line_indent;
                    for k in (0..j).rev() {
                        let pl = &block.lines[k];
                        if pl.trim_start().starts_with('*') || pl.trim_start().starts_with('-') {
                            if pl.contains("[[") {
                                prev_link_indent = pl.len() - pl.trim_start().len();
                                break;
                            }
                        }
                    }

                    if line_indent <= prev_link_indent {
                        continue;
                    }
                }

                if should_break {
                    continue;
                }

                let is_link_only = LINK_ONLY_RE.is_match(line);
                let has_multiple_links = line.matches("[[").count() > 1;

                if is_link_only && !has_multiple_links {
                    continue;
                }

                if is_link_only && has_multiple_links {
                    let trimmed = line.trim();
                    let current_indent = line.len() - line.trim_start().len();
                    let adjusted_indent = current_indent.saturating_sub(base_indent_for_children);
                    section.push_str(&" ".repeat(adjusted_indent));
                    section.push_str(trimmed);
                    section.push('\n');
                    last_was_empty = false;
                    continue;
                }

                if last_was_empty && empty_line_count >= 2 {
                    section.push_str(&" ".repeat(base_indent_for_children));
                    section.push_str("...\n");
                } else if last_was_empty {
                    for _ in 0..empty_line_count {
                        section.push_str(&" ".repeat(base_indent_for_children));
                        section.push('\n');
                    }
                }

                empty_line_count = 0;
                let adjusted_indent = current_indent.saturating_sub(base_indent_for_children);
                section.push_str(&" ".repeat(adjusted_indent));
                section.push_str(trimmed);
                section.push('\n');
                last_was_empty = false;

                if should_break {
                    break;
                }
            }
        }
    }

    section.push_str("\n<!-- BACKLINKS:END -->\n");
    section
}

pub fn rewrite_file_with_backlinks(path: &Path, backlinks: Vec<Backlink>) -> Result<(), String> {
    let content = read_file_content(path)?;
    let clean_content = remove_existing_section(&content);

    let section = build_backlink_section(&backlinks);
    let section_body = section.trim_start();

    let new_content = if clean_content.is_empty() {
        section_body.to_string()
    } else if clean_content.ends_with('\n') {
        format!("{}{}", clean_content, section_body)
    } else {
        format!("{}\n{}", clean_content, section_body)
    };

    if content == new_content {
        return Ok(());
    }

    write_file_content(path, &new_content)
}

pub fn skip_backlink_section(content: &str) -> String {
    BACKLINKS_SECTION_RE.replace_all(content, "").to_string()
}

pub fn extract_blocks(content: &str, source_file: &str) -> Vec<Block> {
    extract_blocks_with_iter(content, source_file)
}

fn extract_blocks_with_iter(content: &str, source_file: &str) -> Vec<Block> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if let Some(captures) = BLOCK_START_RE.captures(line) {
            let indent_level = captures.get(1).map(|m| m.as_str().len()).unwrap_or(0);
            let mut block_lines = vec![line.to_string()];
            i += 1;

            while i < lines.len() {
                let next_line = lines[i];

                if let Some(next_captures) = BLOCK_START_RE.captures(next_line) {
                    let next_indent = next_captures.get(1).map(|m| m.as_str().len()).unwrap_or(0);
                    if next_indent <= indent_level {
                        break;
                    }
                }

                block_lines.push(next_line.to_string());
                i += 1;
            }

            blocks.push(Block::new(source_file, indent_level, block_lines));
        } else {
            i += 1;
        }
    }

    blocks
}

pub fn collect_backlinks(
    target_filename: &str,
    files: &[(String, std::path::PathBuf)],
) -> Vec<Backlink> {
    collect_backlinks_parallel(target_filename, files)
}

fn collect_backlinks_parallel(
    target_filename: &str,
    files: &[(String, std::path::PathBuf)],
) -> Vec<Backlink> {
    let file_results: Vec<(String, Vec<Block>)> = files
        .par_iter()
        .filter_map(|(filename, file_path)| {
            if !file_path.exists() {
                return None;
            }

            let content = match read_file_content(file_path) {
                Ok(c) => c,
                Err(_) => return None,
            };

            let clean_content = skip_backlink_section(&content);
            let blocks = extract_blocks(&clean_content, filename);

            let matching_blocks: Vec<Block> = blocks
                .into_iter()
                .filter(|block| {
                    let links = block.extract_links();
                    links.iter().any(|l| l.target == target_filename)
                })
                .collect();

            if matching_blocks.is_empty() {
                None
            } else {
                Some((filename.clone(), matching_blocks))
            }
        })
        .collect();

    let mut backlinks_map: std::collections::HashMap<String, Vec<Block>> =
        std::collections::HashMap::new();

    for (filename, blocks) in file_results {
        backlinks_map
            .entry(filename)
            .or_insert_with(Vec::new)
            .extend(blocks);
    }

    let mut backlinks: Vec<Backlink> = backlinks_map
        .into_iter()
        .map(|(filename, blocks)| {
            let mut backlink = Backlink::new(&filename);
            for block in blocks {
                backlink.add_block(block);
            }
            backlink
        })
        .collect();

    backlinks.sort_by(|a, b| b.source_file.cmp(&a.source_file));
    backlinks
}

pub fn collect_backlinks_using_index(
    target_filename: &str,
    index: &LinkIndex,
    files: &[(String, std::path::PathBuf)],
) -> Vec<Backlink> {
    let mut source_files = index.get_sources_for_link(target_filename);
    source_files.sort();
    source_files.dedup();

    let files_map: std::collections::HashMap<String, PathBuf> = files
        .iter()
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect();

    let mut backlinks: Vec<Backlink> = source_files
        .into_iter()
        .map(|source| {
            let blocks = extract_blocks_for_source(source, target_filename, &files_map);
            let mut backlink = Backlink::new(source);
            for block in blocks {
                backlink.add_block(block);
            }
            backlink
        })
        .filter(|b| !b.blocks.is_empty())
        .collect();

    backlinks.sort_by(|a, b| b.source_file.cmp(&a.source_file));
    backlinks
}

pub fn collect_backlinks_using_index_with_duplicates(
    target_filename: &str,
    index: &LinkIndex,
    files: &[(String, std::path::PathBuf)],
) -> Vec<Backlink> {
    let source_files = index.get_sources_for_link_with_duplicates(target_filename);

    let files_map: std::collections::HashMap<String, PathBuf> = files
        .iter()
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect();

    let mut backlinks_map: std::collections::HashMap<String, Vec<Block>> =
        std::collections::HashMap::new();

    for source in source_files {
        let blocks = extract_blocks_for_source(source, target_filename, &files_map);
        if !blocks.is_empty() {
            backlinks_map
                .entry(source.to_string())
                .or_insert_with(Vec::new)
                .extend(blocks);
        }
    }

    let mut backlinks: Vec<Backlink> = backlinks_map
        .into_iter()
        .map(|(filename, blocks)| {
            let mut backlink = Backlink::new(&filename);
            for block in blocks {
                backlink.add_block(block);
            }
            backlink
        })
        .collect();

    backlinks.sort_by(|a, b| b.source_file.cmp(&a.source_file));
    backlinks
}

fn extract_blocks_for_source(
    source_file: &str,
    target_filename: &str,
    files_map: &std::collections::HashMap<String, PathBuf>,
) -> Vec<Block> {
    if let Some(path) = files_map.get(source_file) {
        if let Ok(content) = read_file_content(path) {
            let clean_content = skip_backlink_section(&content);
            return extract_blocks(&clean_content, source_file)
                .into_iter()
                .filter(|block| {
                    block
                        .extract_links()
                        .iter()
                        .any(|l| l.target == target_filename)
                })
                .collect();
        }
    }

    Vec::new()
}

pub fn generate_backlinks(vault_path: &str, target_filename: &str) -> Result<(), String> {
    let vault = Path::new(vault_path);
    if !vault.exists() {
        return Err(format!("Invalid vault path: {}", vault_path));
    }

    let files = scan_vault_files(vault);

    let target_path = files
        .iter()
        .find(|(name, _)| name == target_filename)
        .map(|(_, path)| path.clone());

    let (target_path, slug, files) = match target_path {
        Some(path) => (path, target_filename.to_string(), files),
        None => {
            let slug = target_filename.replace(' ', "-").to_lowercase();
            let new_path = Path::new(vault_path).join(format!("{}.md", &slug));
            fs::write(&new_path, format!("# {}", target_filename))
                .map_err(|e| format!("Failed to create file: {}", e))?;
            println!("Created new file: {}.md", slug);
            let new_files = scan_vault_files(vault);
            (new_path, slug, new_files)
        }
    };

    let mut all_backlinks = collect_backlinks(&slug, &files);
    if slug != target_filename {
        let more_backlinks = collect_backlinks(target_filename, &files);
        for bl in more_backlinks {
            if let Some(existing) = all_backlinks
                .iter_mut()
                .find(|b| b.source_file == bl.source_file)
            {
                for block in bl.blocks {
                    existing.add_block(block);
                }
            } else {
                all_backlinks.push(bl);
            }
        }
    }
    let count = all_backlinks.len();

    rewrite_file_with_backlinks(&target_path, all_backlinks)?;

    println!("Generated backlinks for {} ({} sources)", slug, count);
    Ok(())
}

pub fn run(config: &AgendaConfig, target_filename: Option<&str>) {
    let vault_path = config.vault_dir_string();
    let vault = Path::new(&vault_path);

    if let Some(target) = target_filename {
        match generate_backlinks(&vault_path, target) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        let files = scan_vault_files(vault);
        let index = LinkIndex::build(&files);

        let all_targets: Vec<String> = index
            .links_to_sources
            .keys()
            .filter(|t| !files.iter().any(|(n, _)| n == *t))
            .cloned()
            .collect();

        for target in &all_targets {
            let slug = target.replace(' ', "-").to_lowercase();
            let new_path = vault.join(format!("{}.md", &slug));
            if let Err(e) = fs::write(&new_path, format!("# {}", target)) {
                eprintln!("Failed to create file: {}", e);
                continue;
            }
            println!("Created new file: {}.md", slug);
        }

        if !all_targets.is_empty() {
            let files = scan_vault_files(vault);
            println!("Processing {} files in vault", files.len());
            let index = LinkIndex::build(&files);
            for (filename, file_path) in &files {
                let backlinks =
                    collect_backlinks_using_index_with_duplicates(filename, &index, &files);
                let count = backlinks.len();

                match rewrite_file_with_backlinks(file_path, backlinks) {
                    Ok(()) => {
                        if count > 0 {
                            println!("Generated backlinks for {} ({} sources)", filename, count);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error generating backlinks for {}: {}", filename, e);
                    }
                }
            }
        } else {
            for (filename, file_path) in &files {
                let backlinks =
                    collect_backlinks_using_index_with_duplicates(filename, &index, &files);
                let count = backlinks.len();

                match rewrite_file_with_backlinks(file_path, backlinks) {
                    Ok(()) => {
                        if count > 0 {
                            println!("Generated backlinks for {} ({} sources)", filename, count);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error generating backlinks for {}: {}", filename, e);
                    }
                }
            }
        }
    }
}

pub fn generate_backlinks_for_file(
    vault_path: &str,
    target_file_path: &Path,
    target_filename: &str,
) -> Result<(), String> {
    let vault = Path::new(vault_path);
    if !vault.exists() {
        return Err(format!("Invalid vault path: {}", vault_path));
    }

    if !target_file_path.exists() {
        return Err(format!("Missing target file: {:?}", target_file_path));
    }

    let files = scan_vault_files(vault);
    let backlinks = collect_backlinks(target_filename, &files);
    let count = backlinks.len();

    rewrite_file_with_backlinks(target_file_path, backlinks)?;

    println!(
        "Generated backlinks for {} ({} sources)",
        target_filename, count
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_single() {
        let block = Block::new("test.md", 0, vec!["- [[target]]".to_string()]);
        let links = block.extract_links();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target, "target");
    }

    #[test]
    fn test_extract_links_multiple() {
        let block = Block::new(
            "test.md",
            0,
            vec!["- [[target1]] and [[target2]]".to_string()],
        );
        let links = block.extract_links();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "target1");
        assert_eq!(links[1].target, "target2");
    }

    #[test]
    fn test_extract_links_separate_bullets() {
        let block = Block::new(
            "test.md",
            0,
            vec!["- [[target1]]".to_string(), "- [[target2]]".to_string()],
        );
        let links = block.extract_links();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "target1");
        assert_eq!(links[1].target, "target2");
    }

    #[test]
    fn test_extract_links_mixed_bullets() {
        let block = Block::new(
            "test.md",
            0,
            vec!["- [[target]]".to_string(), "- [[other]]".to_string()],
        );
        let links = block.extract_links();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "target");
        assert_eq!(links[1].target, "other");
    }

    #[test]
    fn test_extract_links_none() {
        let block = Block::new("test.md", 0, vec!["- regular bullet".to_string()]);
        let links = block.extract_links();
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_blocks_single() {
        let content = "- First block\n- Second block";
        let blocks = extract_blocks(content, "test.md");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].lines[0], "- First block");
        assert_eq!(blocks[1].lines[0], "- Second block");
    }

    #[test]
    fn test_extract_blocks_with_indentation() {
        let content = "- Parent\n  - Child\n  - Another child";
        let blocks = extract_blocks(content, "test.md");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].lines.len(), 3);
    }

    #[test]
    fn test_extract_blocks_empty() {
        let content = "No bullets here";
        let blocks = extract_blocks(content, "test.md");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_skip_backlink_section() {
        let content = "Some content\n<!-- BACKLINKS:START -->\n## Backlinks\n<!-- BACKLINKS:END -->\nMore content";
        let result = skip_backlink_section(content);
        assert!(!result.contains("BACKLINKS"));
        assert!(result.contains("Some content"));
        assert!(result.contains("More content"));
    }

    #[test]
    fn test_skip_backlink_section_none() {
        let content = "No backlink section here";
        let result = skip_backlink_section(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_remove_existing_section() {
        let content = "Content\n<!-- BACKLINKS:START -->\n## Backlinks\n- [[link]]\n<!-- BACKLINKS:END -->\nEnd";
        let result = remove_existing_section(content);
        assert!(!result.contains("BACKLINKS"));
        assert!(result.contains("Content"));
        assert!(result.contains("End"));
    }

    #[test]
    fn test_remove_existing_section_none() {
        let content = "Just normal content";
        let result = remove_existing_section(content);
        assert_eq!(result, "Just normal content");
    }

    #[test]
    fn test_build_backlink_section_skips_link_only_line() {
        let mut backlink = Backlink::new("source");
        backlink.add_block(Block::new(
            "source",
            0,
            vec!["- [[target]]".to_string(), "  Some context".to_string()],
        ));
        let backlinks = vec![backlink];
        let section = build_backlink_section(&backlinks);
        assert!(section.contains("- [[source]]"));
        assert!(!section.contains("[[target]]"));
        assert!(section.contains("  Some context"));
    }

    #[test]
    fn test_build_backlink_section_adjusts_indentation() {
        let mut backlink = Backlink::new("source");
        backlink.add_block(Block::new(
            "source",
            0,
            vec!["- [[target]]".to_string(), "  - child item".to_string()],
        ));
        let backlinks = vec![backlink];
        let section = build_backlink_section(&backlinks);
        assert!(section.contains("  - child item"));
    }

    #[test]
    fn test_build_backlink_section_with_backlinks() {
        let mut backlink = Backlink::new("source.md");
        backlink.add_block(Block::new(
            "source.md",
            0,
            vec!["-引用".to_string(), "  Some description".to_string()],
        ));
        let backlinks = vec![backlink];
        let section = build_backlink_section(&backlinks);
        assert!(section.contains("- [[source.md]]"));
        assert!(section.contains("Some description"));
    }

    #[test]
    fn test_collect_backlinks_finds_link() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_vault_collect");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(&source_path, "- [[target]]").unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let backlinks = collect_backlinks("target", &files);

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].source_file, "source");

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_collect_backlinks_filters_by_target() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_vault_filter");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(&source_path, "- [[other-target]]").unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let backlinks = collect_backlinks("target", &files);

        assert!(backlinks.is_empty());

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_scan_vault_files() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_vault_scan");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        std::fs::write(vault_path.join("file1.md"), "# File 1").unwrap();
        std::fs::write(vault_path.join("file2.md"), "# File 2").unwrap();
        std::fs::write(vault_path.join("file3.txt"), "Not markdown").unwrap();

        let files = scan_vault_files(&vault_path);
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|(name, _)| name == "file1"));
        assert!(files.iter().any(|(name, _)| name == "file2"));
        assert!(!files.iter().any(|(name, _)| name == "file3"));

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_build_backlink_section_no_extra_lines_for_link_only() {
        let mut backlink = Backlink::new("link-only");
        backlink.add_block(Block::new("link-only", 0, vec!["- [[target]]".to_string()]));

        let section = build_backlink_section(&[backlink]);

        let after_link = section.find("[[link-only]]").map(|i| &section[i..]);
        if let Some(s) = after_link {
            assert!(
                !s.contains("\n\n"),
                "link-only backlink should not have extra blank lines after link"
            );
        }
    }

    #[test]
    fn test_build_backlink_section_ellipsis_for_gaps() {
        let mut backlink = Backlink::new("with-gaps");
        backlink.add_block(Block::new(
            "with-gaps",
            0,
            vec![
                "- [[target]]".to_string(),
                "  context line".to_string(),
                "".to_string(),
                "".to_string(),
                "  more context".to_string(),
            ],
        ));

        let section = build_backlink_section(&[backlink]);

        assert!(section.contains("..."), "should use ellipsis for gaps");
        assert!(
            !section.contains("\n\n\n"),
            "should not preserve multiple empty lines"
        );
    }

    #[test]
    fn test_build_backlink_section_no_ellipsis_for_whitespace_only() {
        let mut backlink = Backlink::new("test");
        backlink.add_block(Block::new(
            "test",
            0,
            vec![
                "- [[target]]".to_string(),
                "  context".to_string(),
                "".to_string(),
                "  ".to_string(),
                "  more context".to_string(),
            ],
        ));

        let section = build_backlink_section(&[backlink]);

        assert!(
            !section.contains("..."),
            "should not use ellipsis for whitespace-only lines"
        );
    }

    #[test]
    fn test_build_backlink_section_no_ellipsis_for_single_empty_line() {
        let mut backlink = Backlink::new("test");
        backlink.add_block(Block::new(
            "test",
            0,
            vec![
                "- [[target]]".to_string(),
                "  context".to_string(),
                "".to_string(),
                "  more context".to_string(),
            ],
        ));

        let section = build_backlink_section(&[backlink]);

        assert!(
            !section.contains("..."),
            "should not use ellipsis for single empty line"
        );
    }

    #[test]
    fn test_build_backlink_section_ellipsis_only_when_deeper_nesting() {
        let mut backlink = Backlink::new("test");
        backlink.add_block(Block::new(
            "test",
            0,
            vec![
                "- [[target]]".to_string(),
                "  context".to_string(),
                "".to_string(),
                "".to_string(),
                "  sibling".to_string(),
            ],
        ));

        let section = build_backlink_section(&[backlink]);

        assert!(
            section.contains("..."),
            "should use ellipsis for gaps with same-level content"
        );
    }

    #[test]
    fn test_build_backlink_section_ellipsis_when_deeper_nesting() {
        let mut backlink = Backlink::new("test");
        backlink.add_block(Block::new(
            "test",
            0,
            vec![
                "- [[target]]".to_string(),
                "  context".to_string(),
                "".to_string(),
                "".to_string(),
                "    deeply nested".to_string(),
            ],
        ));

        let section = build_backlink_section(&[backlink]);

        assert!(
            section.contains("..."),
            "should use ellipsis when following content is deeper nested"
        );
    }

    #[test]
    fn test_build_backlink_section_no_ellipsis_nothing_under_link() {
        let mut backlink = Backlink::new("test");
        backlink.add_block(Block::new("test", 0, vec!["- [[target]]".to_string()]));

        let section = build_backlink_section(&[backlink]);

        assert!(
            !section.contains("..."),
            "should not use ellipsis when nothing under link"
        );
        assert!(
            section.contains("- [[test]]"),
            "should still include the link"
        );
    }

    #[test]
    fn test_generate_backlinks_invalid_vault() {
        let result = generate_backlinks("/nonexistent/path", "target");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid vault path"));
    }

    #[test]
    fn test_collect_backlinks_index_vs_parallel_parity() {
        let vault_path = Path::new("test-vault");
        let files = scan_vault_files(vault_path);
        let index = LinkIndex::build(&files);

        let parallel_backlinks = collect_backlinks("target", &files);
        let index_backlinks = collect_backlinks_using_index("target", &index, &files);

        assert_eq!(
            parallel_backlinks.len(),
            index_backlinks.len(),
            "same number of source files"
        );

        let parallel_sources: Vec<_> = parallel_backlinks.iter().map(|b| &b.source_file).collect();
        let index_sources: Vec<_> = index_backlinks.iter().map(|b| &b.source_file).collect();
        assert_eq!(parallel_sources, index_sources, "same source files");

        for (p, i) in parallel_backlinks.iter().zip(index_backlinks.iter()) {
            assert_eq!(
                p.blocks.len(),
                i.blocks.len(),
                "same number of blocks per source"
            );
        }
    }

    #[test]
    fn test_collect_backlinks_index_excludes_unrelated_blocks() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_exclude_unrelated");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(
            &source_path,
            "- [[target]]\n- unrelated\n  - still unrelated",
        )
        .unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let index = LinkIndex::build(&files);
        let backlinks = collect_backlinks_using_index("target", &index, &files);

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].blocks.len(), 1);

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_collect_backlinks_index_partial_matches() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_partial_matches");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(&source_path, "- [[target]]\n- [[other]]\n- [[target]]").unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let index = LinkIndex::build(&files);
        let backlinks = collect_backlinks_using_index("target", &index, &files);

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].blocks.len(), 2);

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_collect_backlinks_index_no_unrelated_blocks_regression() {
        let vault_path = Path::new("test-vault");
        let files = scan_vault_files(vault_path);
        let index = LinkIndex::build(&files);
        let backlinks = collect_backlinks_using_index("target", &index, &files);

        for backlink in &backlinks {
            for block in &backlink.blocks {
                let links = block.extract_links();
                assert!(
                    links.iter().any(|l| l.target == "target"),
                    "block from {} should contain [[target]], but got: {:?}",
                    backlink.source_file,
                    block.content()
                );
            }
        }
    }

    #[test]
    fn test_collect_backlinks_index_mixed_content() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_mixed_content");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(
            &source_path,
            "- first block [[target]]\n- second block [[other]]\n- third block [[target]] again",
        )
        .unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let index = LinkIndex::build(&files);
        let backlinks = collect_backlinks_using_index("target", &index, &files);

        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].blocks.len(), 2);

        std::fs::remove_dir_all(&vault_path).ok();
    }

    #[test]
    fn test_collect_backlinks_index_empty_result() {
        let temp_dir = std::env::temp_dir();
        let vault_path = temp_dir.join("test_empty_result");
        if vault_path.exists() {
            std::fs::remove_dir_all(&vault_path).ok();
        }
        std::fs::create_dir_all(&vault_path).unwrap();

        let source_path = vault_path.join("source.md");
        let target_path = vault_path.join("target.md");
        std::fs::write(&source_path, "- [[other-target]]").unwrap();
        std::fs::write(&target_path, "# Target").unwrap();

        let files: Vec<(String, std::path::PathBuf)> = vec![
            ("source".to_string(), source_path),
            ("target".to_string(), target_path),
        ];
        let index = LinkIndex::build(&files);
        let backlinks = collect_backlinks_using_index("target", &index, &files);

        assert!(backlinks.is_empty());

        std::fs::remove_dir_all(&vault_path).ok();
    }
}
