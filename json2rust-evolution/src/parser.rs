use anyhow::Result;
use syn::{parse_str, Item, ItemStruct, ItemEnum, Type, Field, Fields};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub name: String,
    pub kind: TypeKind,
    pub span: Option<(usize, usize)>, // (start, end) byte positions in source
}

#[derive(Debug, Clone)]
pub enum TypeKind {
    Struct {
        fields: Vec<FieldInfo>,
    },
    Enum {
        info: EnumInfo,
    },
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub field_type: String,
    pub is_optional: bool,
}

#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Option<Vec<FieldInfo>>,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub variants: Vec<VariantInfo>,
    pub is_untagged: bool,
}

pub struct RustParser {
    source_code: Option<String>,
}

impl RustParser {
    pub fn new() -> Self {
        Self { source_code: None }
    }
    
    pub fn parse_types(&mut self, code: &str) -> Result<HashMap<String, TypeInfo>> {
        self.source_code = Some(code.to_string());
        let mut types = HashMap::new();
        
        if code.trim().is_empty() {
            return Ok(types);
        }
        
        let syntax_tree = parse_str::<syn::File>(code)?;
        
        for item in syntax_tree.items {
            match item {
                Item::Struct(item_struct) => {
                    let type_info = self.parse_struct(item_struct)?;
                    types.insert(type_info.name.clone(), type_info);
                }
                Item::Enum(item_enum) => {
                    let type_info = self.parse_enum(item_enum)?;
                    types.insert(type_info.name.clone(), type_info);
                }
                _ => {} // Ignore other items for now
            }
        }
        
        Ok(types)
    }
    
    fn parse_struct(&self, item_struct: ItemStruct) -> Result<TypeInfo> {
        let name = item_struct.ident.to_string();
        let fields = match item_struct.fields {
            Fields::Named(fields_named) => {
                fields_named.named.iter()
                    .map(|field| self.parse_field(field))
                    .collect::<Result<Vec<_>>>()?
            }
            Fields::Unnamed(_) => Vec::new(), // TODO: Handle tuple structs
            Fields::Unit => Vec::new(),
        };
        
        let span = self.find_type_span(&name, true);
        
        Ok(TypeInfo {
            name,
            kind: TypeKind::Struct { fields },
            span,
        })
    }
    
    fn parse_enum(&self, item_enum: ItemEnum) -> Result<TypeInfo> {
        let name = item_enum.ident.to_string();
        
        // Check for #[serde(untagged)] attribute
        let is_untagged = item_enum.attrs.iter().any(|attr| {
            if attr.path().is_ident("serde") {
                if let Ok(meta_list) = attr.meta.require_list() {
                    return meta_list.tokens.to_string().contains("untagged");
                }
            }
            false
        });
        
        let variants = item_enum.variants.iter()
            .map(|variant| {
                let variant_name = variant.ident.to_string();
                let fields = match &variant.fields {
                    Fields::Named(fields_named) => {
                        Some(fields_named.named.iter()
                            .map(|field| self.parse_field(field))
                            .collect::<Result<Vec<_>>>()?)
                    }
                    Fields::Unnamed(_) => None, // TODO: Handle tuple variants
                    Fields::Unit => None,
                };
                
                Ok(VariantInfo {
                    name: variant_name,
                    fields,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        
        let span = self.find_type_span(&name, false);
        
        Ok(TypeInfo {
            name,
            kind: TypeKind::Enum { 
                info: EnumInfo {
                    variants,
                    is_untagged,
                }
            },
            span,
        })
    }
    
    fn parse_field(&self, field: &Field) -> Result<FieldInfo> {
        let name = field.ident.as_ref()
            .map(|ident| ident.to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        
        let (field_type, is_optional) = self.parse_field_type(&field.ty);
        
        Ok(FieldInfo {
            name,
            field_type,
            is_optional,
        })
    }
    
    fn parse_field_type(&self, ty: &Type) -> (String, bool) {
        match ty {
            Type::Path(type_path) => {
                let type_str = quote::quote! { #type_path }.to_string();
                
                // Check if it's an Option<T>
                if type_str.starts_with("Option <") {
                    (type_str, true)
                } else {
                    (type_str, false)
                }
            }
            _ => (quote::quote! { #ty }.to_string(), false),
        }
    }
    
    pub fn find_type_span(&self, type_name: &str, is_struct: bool) -> Option<(usize, usize)> {
        let source = self.source_code.as_ref()?;
        
        // Look for the type definition - be more flexible with the pattern
        let keyword = if is_struct { "struct" } else { "enum" };
        
        // Try different patterns to find the type
        let patterns = [
            format!("pub {} {}", keyword, type_name),
            format!("{} {}", keyword, type_name),
            format!("pub(crate) {} {}", keyword, type_name),
        ];
        
        for pattern in &patterns {
            if let Some(start_pos) = source.find(pattern) {
                // Look backwards to find the actual start (including attributes)
                let actual_start = self.find_definition_start(source, start_pos)?;
                
                // Find the end of the definition by looking for the closing brace
                let actual_end = self.find_definition_end(source, start_pos)?;
                
                return Some((actual_start, actual_end));
            }
        }
        
        None
    }
    
    fn find_definition_start(&self, source: &str, keyword_pos: usize) -> Option<usize> {
        let before = &source[..keyword_pos];
        let mut lines: Vec<&str> = before.lines().collect();
        let mut current_pos = keyword_pos;
        
        // Go backwards to find attributes and doc comments
        while let Some(line) = lines.pop() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[") || 
               trimmed.starts_with("///") || 
               trimmed.starts_with("//!") ||
               trimmed.starts_with("/**") ||
               trimmed.is_empty() {
                // Find the start of this line
                if let Some(line_start) = before.rfind(line) {
                    current_pos = line_start;
                }
            } else {
                break;
            }
        }
        
        Some(current_pos)
    }
    
    fn find_definition_end(&self, source: &str, start_pos: usize) -> Option<usize> {
        let mut brace_count = 0;
        let mut found_opening = false;
        let mut end_pos = start_pos;
        
        for (i, ch) in source[start_pos..].char_indices() {
            match ch {
                '{' => {
                    brace_count += 1;
                    found_opening = true;
                }
                '}' => {
                    brace_count -= 1;
                    if found_opening && brace_count == 0 {
                        end_pos = start_pos + i + 1;
                        // Include the newline after the closing brace if present
                        if let Some('\n') = source.chars().nth(end_pos) {
                            end_pos += 1;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        
        Some(end_pos)
    }
}
