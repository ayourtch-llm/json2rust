use anyhow::Result;
use crate::parser::TypeInfo;

/// Handles surgical replacement of type definitions in source code
pub struct CodeSurgeon {
    original_source: String,
}

impl CodeSurgeon {
    pub fn new(source: String) -> Self {
        Self {
            original_source: source,
        }
    }
    
    /// Replace a specific type definition with new generated code
    pub fn replace_type_definition(&self, type_info: &TypeInfo, new_definition: &str) -> Result<String> {
        if let Some((start, end)) = type_info.span {
            // Find the full definition including any attributes and comments
            let (actual_start, actual_end) = self.find_full_definition_span(start, end)?;
            
            let mut result = String::new();
            result.push_str(&self.original_source[..actual_start]);
            result.push_str(new_definition);
            result.push_str(&self.original_source[actual_end..]);
            
            Ok(result)
        } else {
            // If we don't have span information, just append the new definition
            Ok(format!("{}\n\n{}", self.original_source, new_definition))
        }
    }
    
    /// Replace multiple type definitions in a single pass using sorted spans
    pub fn replace_multiple_type_definitions(&self, replacements: Vec<TypeReplacement>) -> Result<String> {
        if replacements.is_empty() {
            return Ok(self.original_source.clone());
        }
        
        // Collect all valid span-based replacements and sort by start position
        let mut span_replacements: Vec<_> = replacements
            .into_iter()
            .filter_map(|replacement| {
                if let Some((start, end)) = replacement.type_info.span {
                    Some((start, end, replacement.new_code))
                } else {
                    // For now, skip replacements without spans
                    // In a more complete implementation, we'd handle these separately
                    None
                }
            })
            .collect();
        
        // Sort by start position (ascending)
        span_replacements.sort_by_key(|(start, _, _)| *start);
        
        // Validate that spans don't overlap
        for i in 1..span_replacements.len() {
            let (_, prev_end, _) = span_replacements[i - 1];
            let (curr_start, _, _) = span_replacements[i];
            if prev_end > curr_start {
                return Err(anyhow::anyhow!("Overlapping type definitions detected"));
            }
        }
        
        // Build the result by taking text between spans and inserting new code
        let mut result = String::new();
        let mut last_pos = 0;
        
        for (start, end, new_code) in span_replacements {
            // Find the full definition span including attributes and comments
            let (actual_start, actual_end) = self.find_full_definition_span(start, end)?;
            
            // Add text from last position to start of this definition
            result.push_str(&self.original_source[last_pos..actual_start]);
            
            // Add the new code
            result.push_str(&new_code);
            
            // Update last position to end of this definition
            last_pos = actual_end;
        }
        
        // Add remaining text after the last replacement
        result.push_str(&self.original_source[last_pos..]);
        
        Ok(result)
    }
    
    /// Find the full span including attributes, doc comments, etc.
    fn find_full_definition_span(&self, start: usize, end: usize) -> Result<(usize, usize)> {
        let mut actual_start = start;
        let mut actual_end = end;
        
        // Look backwards for attributes and doc comments
        let before_start = &self.original_source[..start];
        let mut lines_before: Vec<&str> = before_start.lines().collect();
        
        // Count back through attributes and doc comments
        while let Some(line) = lines_before.pop() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[") || 
               trimmed.starts_with("///") || 
               trimmed.starts_with("//!") ||
               trimmed.starts_with("/**") ||
               trimmed.is_empty() {
                // This line should be included
                if let Some(line_start) = before_start.rfind(line) {
                    actual_start = line_start;
                }
            } else {
                break;
            }
        }
        
        // Look forward to include any trailing whitespace
        let after_end = &self.original_source[end..];
        for (i, ch) in after_end.char_indices() {
            if ch == '\n' {
                actual_end = end + i + 1;
                break;
            } else if !ch.is_whitespace() {
                break;
            }
        }
        
        Ok((actual_start, actual_end))
    }
    
    /// Get the unmodified source code
    pub fn original_source(&self) -> &str {
        &self.original_source
    }
}

/// Represents a replacement operation for a type definition
#[derive(Debug, Clone)]
pub struct TypeReplacement {
    pub type_info: TypeInfo,
    pub new_code: String,
}
