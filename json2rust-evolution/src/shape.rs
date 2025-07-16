use crate::parser::{TypeInfo, TypeKind, FieldInfo, VariantInfo};
use std::collections::{HashMap, HashSet};
use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Shape {
    pub fields: Vec<ShapeField>,
    pub metadata: ShapeMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapeMetadata {
    pub original_enum_field_name: Option<String>,  // Track the original field name that contained an enum
    pub source_enum_type: Option<String>,          // Track which enum type this shape came from
}

impl ShapeMetadata {
    pub fn new() -> Self {
        Self {
            original_enum_field_name: None,
            source_enum_type: None,
        }
    }
    
    pub fn with_enum_field(field_name: String, enum_type: String) -> Self {
        Self {
            original_enum_field_name: Some(field_name),
            source_enum_type: Some(enum_type),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShapeField {
    pub name: String,
    pub field_type: String,
    pub is_required: bool,
}

pub struct ShapeExpander {
    verbose: bool,
    known_types: HashMap<String, TypeInfo>,
}

impl ShapeExpander {
    pub fn new() -> Self {
        Self { 
            verbose: false,
            known_types: HashMap::new(),
        }
    }
    
    pub fn with_verbose(verbose: bool) -> Self {
        Self { 
            verbose,
            known_types: HashMap::new(),
        }
    }
    
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }
    
    pub fn set_known_types(&mut self, known_types: HashMap<String, TypeInfo>) {
        self.known_types = known_types;
    }
    
    /// Expand a type into all possible shapes
    pub fn expand_type(&self, type_info: &TypeInfo, verbose: bool) -> Result<Vec<Shape>> {
        if verbose {
            println!("🔄 Expanding type: {}", type_info.name);
        }
        
        let shapes = match &type_info.kind {
            TypeKind::Struct { fields } => {
                if verbose {
                    println!("  📦 Expanding struct with {} fields", fields.len());
                }
                self.expand_struct_shapes(fields)
            }
            TypeKind::Enum { info } => {
                if verbose {
                    println!("  🔀 Expanding enum with {} variants{}", 
                        info.variants.len(),
                        if info.is_untagged { " (untagged)" } else { "" });
                }
                self.expand_enum_shapes(&info.variants, info.is_untagged)
            }
        }?;
        
        if verbose {
            println!("  ➡️  Generated {} shape variants", shapes.len());
            for (i, shape) in shapes.iter().enumerate() {
                println!("    Variant {}: {} fields", i + 1, shape.fields.len());
            }
        }
        
        Ok(shapes)
    }
    
    /// Expand struct into all possible shapes by considering optional fields and untagged enum inlining
    pub fn expand_struct_shapes(&self, fields: &[FieldInfo]) -> Result<Vec<Shape>> {
        if self.verbose {
            println!("    🔧 Expanding struct with {} fields:", fields.len());
            for field in fields {
                println!("      - {}: {} ({})", field.name, field.field_type, 
                    if field.is_optional { "optional" } else { "required" });
            }
        }

        // Check if any fields reference untagged enums and need expansion
        let mut base_shapes = vec![Shape { 
            fields: vec![],
            metadata: ShapeMetadata::new(),
        }];
        
        for field in fields {
            let field_type = if field.is_optional {
                self.unwrap_option_type(&field.field_type)
            } else {
                field.field_type.clone()
            };
            
            // Check if this field references an untagged enum
            if let Some(referenced_type) = self.known_types.get(&field_type) {
                if let TypeKind::Enum { info } = &referenced_type.kind {
                    if info.is_untagged {
                        if self.verbose {
                            println!("      🔄 Expanding untagged enum field '{}' of type '{}'", field.name, field_type);
                        }
                        
                        // Expand the untagged enum into its variant shapes
                        let untagged_shapes = self.expand_enum_shapes(&info.variants, true)?;
                        
                        // For each existing base shape, create new shapes by combining with each untagged variant
                        let mut new_shapes = Vec::new();
                        for base_shape in &base_shapes {
                            if field.is_optional {
                                // Optional untagged enum: include the original shape without this field
                                new_shapes.push(base_shape.clone());
                            }
                            
                            // Add shapes with each variant inlined
                            for untagged_shape in &untagged_shapes {
                                let mut new_shape = base_shape.clone();
                                
                                // Preserve the original enum field name for fold-back
                                new_shape.metadata = ShapeMetadata::with_enum_field(
                                    field.name.clone(),
                                    field_type.clone()
                                );
                                
                                if self.verbose {
                                    println!("      📝 Storing metadata for shape: original_enum_field_name='{}', source_enum_type='{}'", 
                                        field.name, field_type);
                                }
                                
                                // Add all fields from the untagged enum variant
                                for variant_field in &untagged_shape.fields {
                                    new_shape.fields.push(ShapeField {
                                        name: variant_field.name.clone(),
                                        field_type: variant_field.field_type.clone(),
                                        is_required: !field.is_optional && variant_field.is_required,
                                    });
                                }
                                new_shapes.push(new_shape);
                            }
                        }
                        base_shapes = new_shapes;
                        continue;
                    }
                }
            }
            
            // Regular field - add to all base shapes
            for shape in &mut base_shapes {
                shape.fields.push(ShapeField {
                    name: field.name.clone(),
                    field_type: field.field_type.clone(),
                    is_required: !field.is_optional,
                });
            }
        }
        
        // Now handle optional fields expansion for non-untagged-enum fields
        let mut final_shapes = Vec::new();
        
        for base_shape in base_shapes {
            let optional_field_indices: Vec<_> = base_shape.fields.iter()
                .enumerate()
                .filter(|(_, field)| !field.is_required)
                .map(|(i, _)| i)
                .collect();
            
            if optional_field_indices.is_empty() {
                // No optional fields, use the base shape as-is
                final_shapes.push(base_shape);
            } else {
                // Generate all combinations of optional fields
                let num_optional = optional_field_indices.len();
                for i in 0..(1 << num_optional) {
                    let mut shape_fields = Vec::new();
                    
                    // Add all required fields
                    for (idx, field) in base_shape.fields.iter().enumerate() {
                        if field.is_required {
                            shape_fields.push(field.clone());
                        } else {
                            // Check if this optional field is included in this combination
                            if let Some(pos) = optional_field_indices.iter().position(|&x| x == idx) {
                                if (i >> pos) & 1 == 1 {
                                    shape_fields.push(ShapeField {
                                        name: field.name.clone(),
                                        field_type: self.unwrap_option_type(&field.field_type),
                                        is_required: true,
                                    });
                                }
                            }
                        }
                    }
                    
                    final_shapes.push(Shape { 
                        fields: shape_fields,
                        metadata: base_shape.metadata.clone(),
                    });
                }
            }
        }
        
        if self.verbose {
            println!("    ✅ Generated {} total shapes", final_shapes.len());
        }
        
        Ok(final_shapes)
    }
    
    /// Expand enum into shapes for each variant
    fn expand_enum_shapes(&self, variants: &[VariantInfo], is_untagged: bool) -> Result<Vec<Shape>> {
        let mut all_shapes = Vec::new();
        
        for variant in variants {
            if let Some(ref variant_fields) = variant.fields {
                // For each variant, expand its fields like a struct
                let variant_shapes = self.expand_struct_shapes(variant_fields)?;
                
                for mut shape in variant_shapes {
                    if !is_untagged {
                        // For tagged enums, add variant discriminator field
                        shape.fields.insert(0, ShapeField {
                            name: "tag".to_string(), // Use "tag" instead of "variant" for serde compatibility
                            field_type: format!("\"{}\"", variant.name),
                            is_required: true,
                        });
                    }
                    // For untagged enums, we don't add a discriminator - the fields themselves distinguish the variants
                    all_shapes.push(shape);
                }
            } else {
                if !is_untagged {
                    // Unit variant - only for tagged enums
                    all_shapes.push(Shape {
                        fields: vec![ShapeField {
                            name: "tag".to_string(),
                            field_type: format!("\"{}\"", variant.name),
                            is_required: true,
                        }],
                        metadata: ShapeMetadata::new(),
                    });
                }
                // For untagged enums with unit variants, we skip them as they can't be distinguished
            }
        }
        
        Ok(all_shapes)
    }
    
    /// Remove Option<T> wrapper to get T
    fn unwrap_option_type(&self, type_str: &str) -> String {
        if type_str.starts_with("Option <") && type_str.ends_with('>') {
            // Extract T from Option<T>
            let inner = &type_str[8..type_str.len()-1].trim();
            inner.to_string()
        } else {
            type_str.to_string()
        }
    }
    
    /// Find common fields across all shapes
    pub fn find_common_fields(&self, shapes: &[Shape]) -> Vec<ShapeField> {
        if shapes.is_empty() {
            return Vec::new();
        }
        
        let mut field_counts: HashMap<String, usize> = HashMap::new();
        let mut field_types: HashMap<String, String> = HashMap::new();
        
        for shape in shapes {
            let mut seen_fields = HashSet::new();
            for field in &shape.fields {
                if seen_fields.insert(&field.name) {
                    *field_counts.entry(field.name.clone()).or_insert(0) += 1;
                    field_types.insert(field.name.clone(), field.field_type.clone());
                }
            }
        }
        
        let num_shapes = shapes.len();
        field_counts.into_iter()
            .filter(|(_, count)| *count == num_shapes)
            .map(|(name, _)| ShapeField {
                field_type: field_types[&name].clone(),
                name,
                is_required: true,
            })
            .collect()
    }
    
    /// Remove common fields from shapes
    pub fn remove_common_fields(&self, shapes: &[Shape], common_fields: &[ShapeField]) -> Vec<Shape> {
        let common_field_names: HashSet<_> = common_fields.iter()
            .map(|f| &f.name)
            .collect();
        
        shapes.iter()
            .map(|shape| Shape {
                fields: shape.fields.iter()
                    .filter(|field| !common_field_names.contains(&field.name))
                    .cloned()
                    .collect(),
                metadata: shape.metadata.clone(),
            })
            .collect()
    }
}
