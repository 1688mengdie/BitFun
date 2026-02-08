use crate::util::string::truncate_string_by_chars;
use std::fs;

#[derive(Debug)]
pub struct ReadFileResult {
    pub start_line: usize,
    pub end_line: usize,
    pub total_lines: usize,
    pub content: String,
}

/// start_line: starts from 1
pub fn read_file(
    file_path: &str,
    start_line: usize,
    limit: usize,
    max_line_chars: usize,
) -> Result<ReadFileResult, String> {
    if start_line == 0 {
        return Err(format!("`start_line` should start from 1",));
    }
    if limit == 0 {
        return Err(format!("`limit` can't be 0"));
    }
    let start_index = start_line - 1;

    let full_content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;

    let lines: Vec<&str> = full_content.lines().collect();
    let total_lines = lines.len();
    if total_lines == 0 {
        return Ok(ReadFileResult {
            start_line: 0,
            end_line: 0,
            total_lines: 0,
            content: String::new(),
        });
    }

    if start_index >= total_lines {
        return Err(format!(
            "`start_line` {} is larger than the number of lines in the file: {}",
            start_line, total_lines
        ));
    }
    let end_index = (start_index + limit).min(total_lines);
    let selected_lines = &lines[start_index..end_index];

    // Truncate long lines and format with line numbers (cat -n format)
    let truncated_lines: Vec<String> = selected_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let line_number = start_index + idx + 1;
            let line_content = if line.chars().count() > max_line_chars {
                format!(
                    "{} [truncated]",
                    truncate_string_by_chars(line, max_line_chars)
                )
            } else {
                line.to_string()
            };
            format!("{:>6}\t{}", line_number, line_content)
        })
        .collect();
    let final_content = truncated_lines.join("\n");
    Ok(ReadFileResult {
        start_line: start_index + 1,
        end_line: end_index,
        total_lines,
        content: final_content,
    })
}
