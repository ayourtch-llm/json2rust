use crate::parser::{TypeInfo, TypeKind, FieldInfo, VariantInfo};
use crate::shape::{Shape, ShapeField, ShapeExpander};
use crate::optimizer::ShapeOptimizer;
use crate::generator::EvolutionResult;
use std::collections::HashMap;
use anyhow::Result;

pub struct ApiEvolution {
    existing_types: HashMap<String, TypeInfo>,
    shape_expander: ShapeExpander,
    optimizer: ShapeOptimizer,
    verbose: bool,
}

impl ApiEvolution {
    pub fn new(existing_types: HashMap<String, TypeInfo>, verbose: bool) -> Self {
        let mut shape_expander = ShapeExpander::with_verbose(verbose);
        shape_expander.set_known_types(existing_types.clone());
        
        let mut optimizer = ShapeOptimizer::new(verbose);
        optimizer.set_known_types(existing_types.clone());
        
        Self {
            existing_types,
            shape_expander,
            optimizer,
            verbose,
        }
    }
     pub fn evolve_with_json(&mut self, json_value: &serde_json::Value, type_name: &str) -> Result<EvolutionResult> {
        if self.verbose {
            println!("🔍 Starting API Evolution for type: {}", type_name);
            println!("📋 Input JSON: {}", serde_json::to_string_pretty(json_value)?);
        }
        
        // First, analyze the JSON to create a basic shape
        let json_shape = self.analyze_json_shape(json_value)?;
        if self.verbose {
            println!("\n📊 JSON Shape Analysis:");
            self.print_shape(&json_shape, "  ");
        }

        // Look for the specifically requested type first
        let requested_type = self.existing_types.get(type_name);
        
        // Start with just the JSON shape
        let mut all_shapes = vec![json_shape.clone()];
        
        if let Some(requested_type) = requested_type {
            if self.verbose {
                println!("\n🎯 Found requested type: {}", requested_type.name);
                self.print_type_info(requested_type);
            }
            
            // Expand the requested type and add those shapes
            let existing_shapes = self.shape_expander.expand_type(&requested_type, self.verbose)?;
            if self.verbose {
                println!("\n📈 Expanded requested type into {} shapes:", existing_shapes.len());
                for (i, shape) in existing_shapes.iter().enumerate() {
                    println!("  Shape {}:", i + 1);
                    self.print_shape(shape, "    ");
                }
            }
            all_shapes.extend(existing_shapes);
        } else {
            if self.verbose {
                println!("\n⚠️  Requested type '{}' not found, looking for best match...", type_name);
            }
            
            // Fall back to finding the best matching type
            let base_type = self.find_best_matching_type(&json_shape);
            
            if let Some(base_type) = base_type {
                if self.verbose {
                    println!("\n🎯 Found matching existing type: {}", base_type.name);
                    self.print_type_info(base_type);
                }
                
                // Smart handling based on type
                match &base_type.kind {
                    TypeKind::Struct { .. } => {
                        let existing_shapes = self.shape_expander.expand_type(&base_type, self.verbose)?;
                        if self.verbose {
                            println!("\n📈 Expanded existing struct into {} shapes:", existing_shapes.len());
                            for (i, shape) in existing_shapes.iter().enumerate() {
                                println!("  Shape {}:", i + 1);
                                self.print_shape(shape, "    ");
                            }
                        }
                        all_shapes.extend(existing_shapes);
                    }
                    TypeKind::Enum { info } => {
                        if self.verbose {
                            println!("\n🧠 Smart enum analysis - finding best variant matches...");
                        }
                        
                        // For enums, find the best matching variants and evolve intelligently
                        let evolved_shapes = self.evolve_with_enum_variants(&json_shape, &info.variants)?;
                        all_shapes.extend(evolved_shapes);
                    }
                }
            } else if self.verbose {
                println!("\n🆕 No existing types found, creating from scratch");
            }
        }
        
        if self.verbose {
            println!("\n🔄 Total shapes before optimization: {}", all_shapes.len());
            for (i, shape) in all_shapes.iter().enumerate() {
                println!("  Combined Shape {}:", i + 1);
                self.print_shape(shape, "    ");
            }
            
            println!("\n⚡ Starting optimization process...");
        }
        
        // Apply the evolution algorithm
        let optimized_result = self.optimizer.optimize_shapes(&all_shapes, type_name)?;
        
        if self.verbose {
            println!("\n✅ Evolution complete!");
        }
        
        Ok(optimized_result)
    }
    
    pub fn analyze_json_shape(&self, value: &serde_json::Value) -> Result<Shape> {
        match value {
            serde_json::Value::Object(map) => {
                let fields = map.iter()
                    .map(|(key, val)| {
                        let field_type = self.infer_json_type(val);
                        ShapeField {
                            name: key.clone(),
                            field_type,
                            is_required: true,
                        }
                    })
                    .collect();
                
                Ok(Shape { 
                    fields,
                    metadata: crate::shape::ShapeMetadata::new(),
                })
            }
            _ => {
                // For non-objects, create a wrapper shape
                Ok(Shape {
                    fields: vec![ShapeField {
                        name: "value".to_string(),
                        field_type: self.infer_json_type(value),
                        is_required: true,
                    }],
                    metadata: crate::shape::ShapeMetadata::new(),
                })
            }
        }
    }
    
    fn infer_json_type(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => "Option<()>".to_string(),
            serde_json::Value::Bool(_) => "bool".to_string(),
            serde_json::Value::Number(n) => {
                if n.is_i64() {
                    "i64".to_string()
                } else if n.is_u64() {
                    "u64".to_string()
                } else {
                    "f64".to_string()
                }
            }
            serde_json::Value::String(_) => "String".to_string(),
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    "Vec<serde_json::Value>".to_string()
                } else {
                    let element_type = self.infer_json_type(&arr[0]);
                    format!("Vec<{}>", element_type)
                }
            }
            serde_json::Value::Object(_) => "serde_json::Map<String, serde_json::Value>".to_string(),
        }
    }
    
    fn find_best_matching_type(&self, target_shape: &Shape) -> Option<&TypeInfo> {
        let mut best_match = None;
        let mut best_score = 0;
        
        for type_info in self.existing_types.values() {
            let score = self.calculate_compatibility_score(type_info, target_shape);
            if score > best_score {
                best_score = score;
                best_match = Some(type_info);
            }
        }
        
        best_match
    }
    
    fn calculate_compatibility_score(&self, type_info: &TypeInfo, target_shape: &Shape) -> usize {
        match &type_info.kind {
            TypeKind::Struct { fields } => {
                self.calculate_struct_compatibility(fields, target_shape)
            }
            TypeKind::Enum { info: _ } => {
                // For enums, we'd need more sophisticated matching
                // For now, return a low score
                1
            }
        }
    }
    
    fn calculate_struct_compatibility(&self, fields: &[FieldInfo], target_shape: &Shape) -> usize {
        let mut score = 0;
        
        for target_field in &target_shape.fields {
            for existing_field in fields {
                if existing_field.name == target_field.name {
                    score += 2; // Field name match
                    
                    if self.types_compatible(&existing_field.field_type, &target_field.field_type) {
                        score += 3; // Type compatibility
                    }
                    break;
                }
            }
        }
        
        score
    }
    
    fn types_compatible(&self, existing_type: &str, target_type: &str) -> bool {
        // Simplified type compatibility check
        existing_type == target_type || 
        existing_type.contains(target_type) ||
        target_type.contains(existing_type)
    }
    
    /// Evolve JSON shape with existing enum variants intelligently
    fn evolve_with_enum_variants(&self, json_shape: &Shape, variants: &[VariantInfo]) -> Result<Vec<Shape>> {
        let mut evolved_shapes = Vec::new();
        
        // Calculate compatibility score for each variant
        let mut variant_scores = Vec::new();
        for variant in variants {
            let score = self.calculate_variant_compatibility(json_shape, variant);
            variant_scores.push((variant, score));
            
            if self.verbose {
                println!("    📊 {} compatibility: {} points", variant.name, score);
            }
        }
        
        // Sort by score (highest first)
        variant_scores.sort_by(|a, b| b.1.cmp(&a.1));
        
        if let Some((best_variant, best_score)) = variant_scores.first() {
            if self.verbose {
                println!("    🏆 Best matching variant: {} (score: {})", best_variant.name, best_score);
            }
            
            if *best_score > 0 {
                // Create evolved shapes based on the best variant
                if let Some(ref variant_fields) = best_variant.fields {
                    // Expand the variant fields into shapes
                    let variant_shapes = self.shape_expander.expand_struct_shapes(variant_fields)?;
                    
                    if self.verbose {
                        println!("    🔄 Expanding best variant into {} shapes", variant_shapes.len());
                    }
                    
                    // For each variant shape, try to merge with JSON shape
                    for (i, variant_shape) in variant_shapes.iter().enumerate() {
                        let merged_shape = self.merge_shapes(&json_shape, &variant_shape);
                        if self.verbose {
                            println!("    🔗 Merged shape {} with JSON:", i + 1);
                            self.print_shape(&merged_shape, "      ");
                        }
                        evolved_shapes.push(merged_shape);
                    }
                } else {
                    // Unit variant - just add the JSON shape
                    if self.verbose {
                        println!("    📦 Unit variant - using JSON shape as-is");
                    }
                    evolved_shapes.push(json_shape.clone());
                }
            } else {
                if self.verbose {
                    println!("    ⚠️  No good variant match - using JSON shape only");
                }
                evolved_shapes.push(json_shape.clone());
            }
        }
        
        // Include other high-scoring variants for richer type evolution
        for (variant, score) in &variant_scores[1..] {
            if *score > 0 && *score >= variant_scores.first().map(|(_, s)| s / 2).unwrap_or(0) {
                if self.verbose {
                    println!("    ➕ Including additional variant: {} (score: {})", variant.name, score);
                }
                
                if let Some(ref variant_fields) = variant.fields {
                    let variant_shapes = self.shape_expander.expand_struct_shapes(variant_fields)?;
                    evolved_shapes.extend(variant_shapes);
                }
            }
        }
        
        Ok(evolved_shapes)
    }
    
    /// Calculate how well a JSON shape matches an enum variant
    fn calculate_variant_compatibility(&self, json_shape: &Shape, variant: &VariantInfo) -> usize {
        let Some(ref variant_fields) = variant.fields else {
            return 0; // Unit variants get no score for field matching
        };
        
        let mut score = 0;
        let mut matched_fields = 0;
        
        for json_field in &json_shape.fields {
            for variant_field in variant_fields {
                if json_field.name == variant_field.name {
                    score += 10; // Field name match is worth 10 points
                    matched_fields += 1;
                    
                    if self.types_compatible(&json_field.field_type, &variant_field.field_type) {
                        score += 5; // Type compatibility is worth 5 more points
                    }
                    break;
                }
            }
        }
        
        // Bonus for having good field coverage
        let variant_field_count = variant_fields.len();
        let coverage = matched_fields as f32 / variant_field_count.max(1) as f32;
        score += (coverage * 10.0) as usize;
        
        // Penalty for having too many unmatched JSON fields
        let unmatched_json_fields = json_shape.fields.len() - matched_fields;
        if unmatched_json_fields > variant_field_count {
            score = score.saturating_sub(unmatched_json_fields * 2);
        }
        
        score
    }
    
    /// Merge two shapes into one, handling overlapping and unique fields intelligently
    fn merge_shapes(&self, json_shape: &Shape, variant_shape: &Shape) -> Shape {
        let mut merged_fields = Vec::new();
        let mut seen_fields = std::collections::HashSet::new();
        
        // Start with JSON fields (they take precedence for field types)
        for field in &json_shape.fields {
            merged_fields.push(field.clone());
            seen_fields.insert(&field.name);
        }
        
        // Add unique fields from variant shape, making them optional since they weren't in JSON
        for field in &variant_shape.fields {
            if !seen_fields.contains(&field.name) {
                let mut variant_field = field.clone();
                // Make variant-only fields optional since they're not in the JSON
                // But avoid double-wrapping if already optional
                if !variant_field.field_type.starts_with("Option<") && !variant_field.field_type.starts_with("Option <") && !variant_field.field_type.contains("Option<") && !variant_field.field_type.contains("Option <") {
                    variant_field.field_type = format!("Option<{}>", variant_field.field_type);
                }
                variant_field.is_required = false;
                merged_fields.push(variant_field);
                seen_fields.insert(&field.name);
            }
        }
        
        Shape { 
            fields: merged_fields,
            metadata: crate::shape::ShapeMetadata::new(),
        }
    }

    // Debug helper methods
    fn print_shape(&self, shape: &Shape, indent: &str) {
        if shape.fields.is_empty() {
            println!("{}Empty shape", indent);
            return;
        }
        
        for field in &shape.fields {
            let required_marker = if field.is_required { "✓" } else { "?" };
            println!("{}{} {}: {} {}", indent, required_marker, field.name, field.field_type, 
                if field.is_required { "(required)" } else { "(optional)" });
        }
    }
    
    fn print_type_info(&self, type_info: &TypeInfo) {
        match &type_info.kind {
            TypeKind::Struct { fields } => {
                println!("  📦 Struct with {} fields:", fields.len());
                for field in fields {
                    let opt_marker = if field.is_optional { "?" } else { "!" };
                    println!("    {} {}: {}", opt_marker, field.name, field.field_type);
                }
            }
            TypeKind::Enum { info } => {
                println!("  🔀 Enum with {} variants:", info.variants.len());
                for variant in &info.variants {
                    match &variant.fields {
                        Some(fields) => println!("    {} ({} fields)", variant.name, fields.len()),
                        None => println!("    {} (unit variant)", variant.name),
                    }
                }
            }
        }
    }
}
