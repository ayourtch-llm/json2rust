use crate::shape::ShapeField;
use crate::optimizer::ShapeVariant;
use anyhow::Result;

#[derive(Debug)]
pub enum EvolutionResult {
    SimpleStruct {
        name: String,
        fields: Vec<ShapeField>,
    },
    ComplexEnum {
        name: String,
        common_fields: Vec<ShapeField>,
        variants: Vec<ShapeVariant>,
    },
    StructWithExtendedEnum {
        struct_name: String,
        struct_fields: Vec<ShapeField>,
        enum_name: String,
        new_enum_variants: Vec<ShapeVariant>,
    },
}

impl EvolutionResult {
    pub fn simple_struct(name: &str, fields: Vec<ShapeField>) -> Self {
        Self::SimpleStruct {
            name: name.to_string(),
            fields,
        }
    }
    
    pub fn complex_enum(name: &str, common_fields: Vec<ShapeField>, variants: Vec<ShapeVariant>) -> Self {
        Self::ComplexEnum {
            name: name.to_string(),
            common_fields,
            variants,
        }
    }
    
    pub fn struct_with_extended_enum(
        struct_name: &str, 
        struct_fields: Vec<ShapeField>, 
        enum_name: &str, 
        new_enum_variants: Vec<ShapeVariant>
    ) -> Self {
        Self::StructWithExtendedEnum {
            struct_name: struct_name.to_string(),
            struct_fields,
            enum_name: enum_name.to_string(),
            new_enum_variants,
        }
    }
    
    pub fn generate_rust_code(&self) -> Result<String> {
        match self {
            Self::SimpleStruct { name, fields } => {
                self.generate_struct_code(name, fields)
            }
            Self::ComplexEnum { name, common_fields, variants } => {
                self.generate_complex_enum_code(name, common_fields, variants)
            }
            Self::StructWithExtendedEnum { struct_name, struct_fields, enum_name, new_enum_variants } => {
                self.generate_struct_with_extended_enum_code(struct_name, struct_fields, enum_name, new_enum_variants)
            }
        }
    }
    
    fn generate_struct_code(&self, name: &str, fields: &[ShapeField]) -> Result<String> {
        let mut code = String::new();
        
        code.push_str(&format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n"));
        code.push_str(&format!("pub struct {} {{\n", name));
        
        for field in fields {
            let field_type = if field.is_required {
                field.field_type.clone()
            } else if field.field_type.starts_with("Option<") || field.field_type.starts_with("Option <") {
                // Field is already optional, don't double-wrap
                field.field_type.clone()
            } else {
                format!("Option<{}>", field.field_type)
            };
            
            code.push_str(&format!("    pub {}: {},\n", field.name, field_type));
        }
        
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn generate_complex_enum_code(&self, name: &str, common_fields: &[ShapeField], variants: &[ShapeVariant]) -> Result<String> {
        let mut code = String::new();
        
        // Check if this is a simple case: common fields + one variant with fields + one empty variant
        // This should become a struct with an Option<SubStruct> field instead of an enum
        if variants.len() == 2 {
            let has_empty_variant = variants.iter().any(|v| v.fields.is_empty());
            let non_empty_variants: Vec<_> = variants.iter().filter(|v| !v.fields.is_empty()).collect();
            
            if has_empty_variant && non_empty_variants.len() == 1 && !common_fields.is_empty() {
                // Generate struct with Option<SubStruct> pattern
                let extra_variant = non_empty_variants[0];
                let extra_struct_name = format!("{}Extra", name);
                
                // Generate the extra struct for non-common fields
                code.push_str(&self.generate_struct_code(&extra_struct_name, &extra_variant.fields)?);
                code.push_str("\n");
                
                // Generate the main struct with common fields + optional extra
                code.push_str(&format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n"));
                code.push_str(&format!("pub struct {} {{\n", name));
                
                // Add common fields
                for field in common_fields {
                    let field_type = if field.is_required {
                        field.field_type.clone()
                    } else if field.field_type.starts_with("Option<") || field.field_type.starts_with("Option <") {
                        field.field_type.clone()
                    } else {
                        format!("Option<{}>", field.field_type)
                    };
                    code.push_str(&format!("    pub {}: {},\n", field.name, field_type));
                }
                
                // Add the optional extra fields as a single Option<SubStruct>
                code.push_str(&format!("    #[serde(flatten)]\n"));
                code.push_str(&format!("    pub extra: Option<{}>,\n", extra_struct_name));
                
                code.push_str("}\n");
                
                return Ok(code);
            }
        }
        
        // Fall back to the original enum-based approach for more complex cases
        if !common_fields.is_empty() {
            // Generate a base struct with common fields
            let base_name = format!("{}Base", name);
            code.push_str(&self.generate_struct_code(&base_name, common_fields)?);
            code.push_str("\n");
        }
        
        // Generate the enum
        code.push_str(&format!("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n"));
        code.push_str(&format!("#[serde(untagged)]\n"));
        code.push_str(&format!("pub enum {} {{\n", name));
        
        for variant in variants {
            code.push_str(&format!("    {} {{\n", variant.name));
            
            // Include common fields if any
            if !common_fields.is_empty() {
                code.push_str(&format!("        #[serde(flatten)]\n"));
                code.push_str(&format!("        base: {}Base,\n", name));
            }
            
            // Add variant-specific fields
            for field in &variant.fields {
                let field_type = if field.is_required {
                    field.field_type.clone()
                } else if field.field_type.starts_with("Option<") || field.field_type.starts_with("Option <") {
                    // Field is already optional, don't double-wrap
                    field.field_type.clone()
                } else {
                    format!("Option<{}>", field.field_type)
                };
                code.push_str(&format!("        {}: {},\n", field.name, field_type));
            }
            
            code.push_str("    },\n");
        }
        
        code.push_str("}\n");
        
        Ok(code)
    }
    
    fn generate_struct_with_extended_enum_code(
        &self, 
        struct_name: &str, 
        struct_fields: &[ShapeField], 
        _enum_name: &str, 
        _new_enum_variants: &[ShapeVariant]
    ) -> Result<String> {
        // For surgical replacement, only generate the struct code
        // The enum extension is handled separately in main.rs
        self.generate_struct_code(struct_name, struct_fields)
    }
}
