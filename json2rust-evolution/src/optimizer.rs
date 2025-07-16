use crate::shape::{Shape, ShapeField};
use crate::generator::EvolutionResult;
use crate::parser::{TypeInfo, TypeKind};
use std::collections::{HashMap, HashSet};
use anyhow::Result;

pub struct ShapeOptimizer {
    verbose: bool,
    known_types: HashMap<String, TypeInfo>,
}

#[derive(Debug, Clone)]
pub struct OptimizedShape {
    pub common_fields: Vec<ShapeField>,
    pub variants: Vec<ShapeVariant>,
}

#[derive(Debug, Clone)]
pub struct ShapeVariant {
    pub name: String,
    pub fields: Vec<ShapeField>,
}

impl ShapeOptimizer {
    pub fn new(verbose: bool) -> Self {
        Self { 
            verbose,
            known_types: HashMap::new(),
        }
    }
    
    pub fn with_known_types(verbose: bool, known_types: HashMap<String, TypeInfo>) -> Self {
        Self { 
            verbose,
            known_types,
        }
    }
    
    pub fn set_known_types(&mut self, known_types: HashMap<String, TypeInfo>) {
        self.known_types = known_types;
    }
    
    pub fn optimize_shapes(&self, shapes: &[Shape], base_name: &str) -> Result<EvolutionResult> {
        if self.verbose {
            println!("\n🎯 Starting shape optimization for: {}", base_name);
            println!("   Input: {} shapes", shapes.len());
        }
        
        if shapes.is_empty() {
            if self.verbose {
                println!("   ⚠️  Empty input - creating empty struct");
            }
            return Ok(EvolutionResult::simple_struct(base_name, Vec::new()));
        }
        
        if shapes.len() == 1 {
            if self.verbose {
                println!("   ✨ Single shape - creating simple struct");
            }
            return Ok(EvolutionResult::simple_struct(base_name, shapes[0].fields.clone()));
        }
        
        // Step 1: Find common fields across all shapes
        if self.verbose {
            println!("\n📊 Step 1: Finding common fields...");
        }
        let common_fields = self.find_common_fields(shapes);
        if self.verbose {
            println!("   Found {} common fields:", common_fields.len());
            for field in &common_fields {
                println!("     ✓ {}: {}", field.name, field.field_type);
            }
        }
        
        // Step 2: Remove common fields from shapes to get remaining variants
        if self.verbose {
            println!("\n🔄 Step 2: Removing common fields from variants...");
        }
        let variant_shapes = self.remove_common_fields(shapes, &common_fields);
        if self.verbose {
            println!("   Variant shapes after common field removal:");
            for (i, shape) in variant_shapes.iter().enumerate() {
                println!("     Variant {}: {} remaining fields", i + 1, shape.fields.len());
                for field in &shape.fields {
                    println!("       - {}: {}", field.name, field.field_type);
                }
            }
        }
        
        // Step 3: Try to optimize variants by finding mergeable shapes
        if self.verbose {
            println!("\n🔧 Step 3: Optimizing variants...");
        }
        let optimized_variants = self.optimize_variants(&variant_shapes)?;
        if self.verbose {
            println!("   Optimized to {} variants:", optimized_variants.len());
            for (i, variant) in optimized_variants.iter().enumerate() {
                println!("     Variant {}: {} (with {} fields)", i + 1, variant.name, variant.fields.len());
            }
        }
        
        // Step 4: Apply recursive optimization if needed
        if self.verbose {
            println!("\n🔄 Step 4: Applying recursive optimization...");
        }
        let final_variants = self.apply_recursive_optimization(&optimized_variants)?;
        
        // Step 5: Try to fold back to existing types
        if self.verbose {
            println!("\n🔄 Step 5: Analyzing fold-back opportunities...");
        }
        let fold_back_result = self.analyze_fold_back(&common_fields, &final_variants, base_name, shapes)?;
        if let Some(folded_result) = fold_back_result {
            if self.verbose {
                println!("   ✅ Successfully folded back to existing type structure");
            }
            return Ok(folded_result);
        }
        
        // Generate the result
        let result = if final_variants.is_empty() {
            if self.verbose {
                println!("\n🎯 Result: Simple struct (no variants)");
            }
            Ok(EvolutionResult::simple_struct(base_name, common_fields))
        } else if common_fields.is_empty() && final_variants.len() == 1 {
            if self.verbose {
                println!("\n🎯 Result: Simple struct (single variant, no common fields)");
            }
            Ok(EvolutionResult::simple_struct(base_name, final_variants[0].fields.clone()))
        } else if final_variants.len() == 1 && final_variants[0].fields.is_empty() {
            if self.verbose {
                println!("\n🎯 Result: Simple struct (single empty variant, all fields common)");
            }
            Ok(EvolutionResult::simple_struct(base_name, common_fields))
        } else if final_variants.len() == 1 {
            if self.verbose {
                println!("\n🎯 Result: Simple struct (single variant merged with common fields)");
            }
            let mut all_fields = common_fields.clone();
            all_fields.extend(final_variants[0].fields.clone());
            let cleaned_fields = self.clean_field_types(all_fields);
            Ok(EvolutionResult::simple_struct(base_name, cleaned_fields))
        } else {
            if self.verbose {
                println!("\n🎯 Result: Complex enum with {} common fields and {} variants", 
                    common_fields.len(), final_variants.len());
            }
            Ok(EvolutionResult::complex_enum(base_name, common_fields, final_variants))
        };
        
        result
    }
    
    fn find_common_fields(&self, shapes: &[Shape]) -> Vec<ShapeField> {
        if shapes.is_empty() {
            return Vec::new();
        }
        
        let mut field_counts: HashMap<String, usize> = HashMap::new();
        let mut field_info: HashMap<String, ShapeField> = HashMap::new();
        
        for shape in shapes {
            let mut seen_in_shape = HashSet::new();
            for field in &shape.fields {
                if seen_in_shape.insert(&field.name) {
                    *field_counts.entry(field.name.clone()).or_insert(0) += 1;
                    
                    // When we encounter a field, check if we already have one stored
                    if let Some(existing_field) = field_info.get(&field.name) {
                        // Choose the better type (prioritize existing over JSON-inferred)
                        let better_field = self.choose_better_field_type(existing_field, field);
                        field_info.insert(field.name.clone(), better_field);
                    } else {
                        field_info.insert(field.name.clone(), field.clone());
                    }
                }
            }
        }
        
        let total_shapes = shapes.len();
        field_counts.into_iter()
            .filter(|(_, count)| *count == total_shapes)
            .map(|(name, _)| field_info[&name].clone())
            .collect()
    }
    
    fn remove_common_fields(&self, shapes: &[Shape], common_fields: &[ShapeField]) -> Vec<Shape> {
        let common_names: HashSet<_> = common_fields.iter()
            .map(|f| &f.name)
            .collect();
        
        shapes.iter()
            .map(|shape| Shape {
                fields: shape.fields.iter()
                    .filter(|field| !common_names.contains(&field.name))
                    .cloned()
                    .collect(),
                metadata: shape.metadata.clone(),
            })
            .collect()
    }
    
    fn optimize_variants(&self, shapes: &[Shape]) -> Result<Vec<ShapeVariant>> {
        if self.verbose {
            println!("   🔧 Optimizing {} variant shapes...", shapes.len());
        }
        
        let mut variants = Vec::new();
        let mut processed = vec![false; shapes.len()];
        
        for i in 0..shapes.len() {
            if processed[i] {
                continue;
            }
            
            let mut mergeable_indices = vec![i];
            processed[i] = true;
            
            if self.verbose {
                println!("     🔍 Processing shape {} as base for merging...", i + 1);
            }
            
            // Find shapes that can be merged with this one
            for j in (i + 1)..shapes.len() {
                if processed[j] {
                    continue;
                }
                
                if self.can_merge_shapes(&shapes[i], &shapes[j]) {
                    if self.verbose {
                        println!("       ✅ Shape {} can merge with shape {}", j + 1, i + 1);
                    }
                    mergeable_indices.push(j);
                    processed[j] = true;
                } else if self.verbose {
                    let diff_count = self.count_field_differences(&shapes[i], &shapes[j]);
                    println!("       ❌ Shape {} cannot merge with shape {} (diff: {})", j + 1, i + 1, diff_count);
                }
            }
            
            // Create a variant from the mergeable shapes
            let variant = self.create_merged_variant(&mergeable_indices, shapes)?;
            if self.verbose {
                println!("     ➡️  Created variant: {} (from {} shapes)", variant.name, mergeable_indices.len());
            }
            variants.push(variant);
        }
        
        if self.verbose {
            println!("   📊 Optimization result: {} variants", variants.len());
        }
        Ok(variants)
    }
    
    fn can_merge_shapes(&self, shape1: &Shape, shape2: &Shape) -> bool {
        // Two shapes can be merged if they differ by exactly one optional field
        let diff_count = self.count_field_differences(shape1, shape2);
        diff_count <= 1
    }
    
    fn count_field_differences(&self, shape1: &Shape, shape2: &Shape) -> usize {
        let fields1: HashSet<_> = shape1.fields.iter().map(|f| &f.name).collect();
        let fields2: HashSet<_> = shape2.fields.iter().map(|f| &f.name).collect();
        
        let diff1: HashSet<_> = fields1.difference(&fields2).collect();
        let diff2: HashSet<_> = fields2.difference(&fields1).collect();
        
        diff1.len() + diff2.len()
    }
    
    fn create_merged_variant(&self, indices: &[usize], shapes: &[Shape]) -> Result<ShapeVariant> {
        if indices.len() == 1 {
            // Single shape variant
            let shape = &shapes[indices[0]];
            return Ok(ShapeVariant {
                name: format!("Variant{}", indices[0] + 1),
                fields: shape.fields.clone(),
            });
        }
        
        // Multiple shapes - find common fields and make others optional
        let variant_shapes: Vec<_> = indices.iter().map(|&i| shapes[i].clone()).collect();
        let common_fields = self.find_common_fields(&variant_shapes);
        let all_fields = self.collect_all_fields(&variant_shapes);
        
        let mut merged_fields = common_fields;
        
        // Add non-common fields as optional
        for field in all_fields {
            if !merged_fields.iter().any(|f| f.name == field.name) {
                let mut optional_field = field;
                // Only wrap in Option if not already optional
                if !optional_field.field_type.starts_with("Option<") && !optional_field.field_type.starts_with("Option <") && !optional_field.field_type.contains("Option<") && !optional_field.field_type.contains("Option <") {
                    optional_field.field_type = format!("Option<{}>", optional_field.field_type);
                }
                merged_fields.push(optional_field);
            }
        }
        
        Ok(ShapeVariant {
            name: format!("MergedVariant{}", indices[0] + 1),
            fields: self.clean_field_types(merged_fields),
        })
    }
    
    fn collect_all_fields(&self, shapes: &[Shape]) -> Vec<ShapeField> {
        let mut all_fields = Vec::new();
        let mut seen_names = HashSet::new();
        
        for shape in shapes {
            for field in &shape.fields {
                if seen_names.insert(&field.name) {
                    all_fields.push(field.clone());
                }
            }
        }
        
        all_fields
    }
    
    fn apply_recursive_optimization(&self, variants: &[ShapeVariant]) -> Result<Vec<ShapeVariant>> {
        // For now, just return the variants as-is
        // In a more sophisticated implementation, we would recursively apply
        // the optimization algorithm to nested structures
        Ok(variants.to_vec())
    }
    
    fn clean_field_types(&self, fields: Vec<ShapeField>) -> Vec<ShapeField> {
        fields.into_iter().map(|mut field| {
            if self.verbose {
                println!("  🧹 Cleaning field type: {} : {}", field.name, field.field_type);
            }
            // Remove double Option wrapping like Option<Option<Type>> -> Option<Type>
            if field.field_type.starts_with("Option<Option<") && field.field_type.ends_with(">>") {
                // Extract the inner type
                let inner = &field.field_type[14..field.field_type.len()-2];
                field.field_type = format!("Option<{}>", inner);
                if self.verbose {
                    println!("    ✨ Fixed double Option: {} -> {}", field.name, field.field_type);
                }
            } else if field.field_type.starts_with("Option < Option <") && field.field_type.ends_with(" > >") {
                // Handle the quote-formatted version with spaces
                let inner = &field.field_type[17..field.field_type.len()-4].trim();
                field.field_type = format!("Option < {} >", inner);
                if self.verbose {
                    println!("    ✨ Fixed double Option (spaced): {} -> {}", field.name, field.field_type);
                }
            }
            field
        }).collect()
    }
    
    /// Analyze if the optimized result can be folded back to use existing types
    fn analyze_fold_back(&self, common_fields: &[ShapeField], variants: &[ShapeVariant], base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        if self.verbose {
            println!("   🔍 Checking for fold-back opportunities...");
        }
        
        // Try to find sub-patterns that could be folded back, even without universal common fields
        if let Some(fold_back_result) = self.try_pattern_fold_back(variants, base_name, original_shapes)? {
            return Ok(Some(fold_back_result));
        }
        
        // Original logic: only attempt fold-back if we have common fields and variants
        if !common_fields.is_empty() && !variants.is_empty() {
            // Look for patterns where the variants could be represented as an existing enum type
            if let Some(fold_back_result) = self.try_enum_fold_back(common_fields, variants, base_name, original_shapes)? {
                return Ok(Some(fold_back_result));
            }
        }
        
        if self.verbose {
            println!("   ❌ No fold-back opportunities found");
        }
        
        Ok(None)
    }
    
    /// Try to find patterns in variants that can be folded back to existing types
    fn try_pattern_fold_back(&self, variants: &[ShapeVariant], base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        if self.verbose {
            println!("     🔍 Analyzing variant patterns for fold-back...");
            println!("       📊 Total variants to analyze: {}", variants.len());
            for (i, variant) in variants.iter().enumerate() {
                let field_names: Vec<String> = variant.fields.iter().map(|f| f.name.clone()).collect();
                println!("         Variant {}: {} ({})", i+1, variant.name, field_names.join(", "));
            }
        }
        
        // Try to find the largest subset of variants that share common fields
        // and whose remaining patterns match existing enum types
        for common_field_threshold in (1..=variants.len()).rev() {
            if self.verbose {
                println!("       🎯 Trying to find {} variants with common fields...", common_field_threshold);
            }
            
            if let Some(fold_back) = self.try_find_common_field_fold_back(variants, common_field_threshold, base_name, original_shapes)? {
                return Ok(Some(fold_back));
            }
        }
        
        Ok(None)
    }
    
    /// Try to find a group of variants with common fields that can be folded back
    fn try_find_common_field_fold_back(&self, variants: &[ShapeVariant], min_variants: usize, base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        if variants.len() < min_variants {
            return Ok(None);
        }
        
        // Find all possible field names across all variants
        let mut all_field_names = HashSet::new();
        for variant in variants {
            for field in &variant.fields {
                all_field_names.insert(&field.name);
            }
        }
        
        if self.verbose {
            println!("         📋 All field names across variants: {:?}", 
                all_field_names.iter().collect::<Vec<_>>());
        }
        
        // For each potential common field, see if enough variants share it
        for field_name in &all_field_names {
            let variants_with_field: Vec<(usize, &ShapeVariant, &ShapeField)> = variants.iter()
                .enumerate()
                .filter_map(|(i, variant)| {
                    variant.fields.iter()
                        .find(|f| &f.name == *field_name)
                        .map(|field| (i, variant, field))
                })
                .collect();
            
            if variants_with_field.len() >= min_variants {
                if self.verbose {
                    println!("         ✅ Field '{}' appears in {} variants (≥ {} required)", 
                        field_name, variants_with_field.len(), min_variants);
                    for (i, _variant, field) in &variants_with_field {
                        println!("           - Variant {}: {} = {}", i+1, field.name, field.field_type);
                    }
                }
                
                // Check if this common field can lead to a fold-back
                let indices: Vec<usize> = variants_with_field.iter().map(|(i, _, _)| *i).collect();
                if let Some(fold_back) = self.try_fold_back_with_common_field(&indices, field_name, variants, base_name, original_shapes)? {
                    return Ok(Some(fold_back));
                }
            } else if self.verbose {
                println!("         ❌ Field '{}' appears in {} variants (< {} required)", 
                    field_name, variants_with_field.len(), min_variants);
            }
        }
        
        Ok(None)
    }
    
    /// Try to fold back variants that share a specific common field
    fn try_fold_back_with_common_field(&self, variant_indices: &[usize], common_field_name: &str, variants: &[ShapeVariant], base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        if self.verbose {
            println!("           🔍 Trying fold-back with common field '{}' across {} variants", common_field_name, variant_indices.len());
        }
        
        // Extract the common field info and remaining patterns
        let mut common_field_info = None;
        let mut remaining_patterns = Vec::new();
        
        for &idx in variant_indices {
            let variant = &variants[idx];
            let mut variant_common_field = None;
            let mut remaining_fields = Vec::new();
            
            for field in &variant.fields {
                if field.name == common_field_name {
                    variant_common_field = Some(field.clone());
                } else {
                    remaining_fields.push(field.clone());
                }
            }
            
            if let Some(common_field) = variant_common_field {
                if common_field_info.is_none() {
                    common_field_info = Some(common_field);
                }
                remaining_patterns.push((idx, remaining_fields));
            }
        }
        
        if let Some(common_field) = common_field_info {
            if self.verbose {
                println!("             🔧 Common field: {} : {}", common_field.name, common_field.field_type);
                println!("             🔍 Checking {} remaining patterns against known enums", remaining_patterns.len());
            }
            
            // Check if the remaining patterns match any existing untagged enum
            for (type_name, type_info) in &self.known_types {
                if let TypeKind::Enum { info } = &type_info.kind {
                    if info.is_untagged {
                        if let Some(compatibility) = self.check_pattern_compatibility(&remaining_patterns, info)? {
                            if compatibility >= 0.7 { // Lower threshold for fold-back
                                if self.verbose {
                                    println!("               ✅ Found compatible enum! {} (score: {:.1}%)", type_name, compatibility * 100.0);
                                }
                                
                                // CRITICAL: Only fold back if ALL variants in the analysis can be represented
                                // Check if we have variants that DON'T match the existing enum
                                let all_variant_indices: Vec<usize> = (0..variants.len()).collect();
                                let non_matching_variants: Vec<usize> = all_variant_indices.iter()
                                    .filter(|&&idx| !variant_indices.contains(&idx))
                                    .copied()
                                    .collect();
                                
                                if !non_matching_variants.is_empty() {
                                    if self.verbose {
                                        println!("               ⚠️  Found {} variants that don't match the existing enum pattern", non_matching_variants.len());
                                        for &idx in &non_matching_variants {
                                            let variant = &variants[idx];
                                            let field_names: Vec<String> = variant.fields.iter().map(|f| f.name.clone()).collect();
                                            println!("                 - Variant {}: {} ({})", idx+1, variant.name, field_names.join(", "));
                                        }
                                        println!("               🔄 Creating mixed structure: folded variants + remaining variants");
                                    }
                                    
                                    // Create a complex enum that includes both the folded-back struct and the remaining variants
                                    return self.create_mixed_fold_back_result(
                                        variant_indices, 
                                        &non_matching_variants, 
                                        &common_field, 
                                        type_name, 
                                        variants, 
                                        base_name,
                                        original_shapes
                                    );
                                }
                                
                                if self.verbose {
                                    println!("               🔄 All variants match - pure fold-back to struct: {} + {}", common_field.name, type_name);
                                }
                                
                                // All variants match - create pure folded-back struct
                                let mut folded_fields = vec![common_field];
                                
                                let enum_field_name = if type_name.to_lowercase().contains("variant") {
                                    type_name.trim_end_matches("Variant").to_lowercase()
                                } else {
                                    format!("{}_type", type_name.to_lowercase())
                                };
                                
                                folded_fields.push(ShapeField {
                                    name: enum_field_name,
                                    field_type: type_name.to_string(),
                                    is_required: true,
                                });
                                
                                return Ok(Some(EvolutionResult::simple_struct(base_name, folded_fields)));
                            }
                        }
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Check how well the remaining patterns match an enum's variants
    fn check_pattern_compatibility(&self, remaining_patterns: &[(usize, Vec<ShapeField>)], enum_info: &crate::parser::EnumInfo) -> Result<Option<f64>> {
        let total_patterns = remaining_patterns.len();
        let mut matched_patterns = 0;
        
        if self.verbose {
            println!("               📊 Compatibility analysis:");
            println!("                 Target enum has {} variants", enum_info.variants.len());
            println!("                 We have {} patterns to match", total_patterns);
        }
        
        for (pattern_idx, (variant_idx, pattern_fields)) in remaining_patterns.iter().enumerate() {
            if self.verbose {
                println!("                 🔍 Pattern {} (from variant {}):", pattern_idx + 1, variant_idx + 1);
                for field in pattern_fields {
                    println!("                   - {}: {} ({})", field.name, field.field_type, 
                        if field.is_required { "required" } else { "optional" });
                }
            }
            
            // Convert pattern to comparable format
            let mut pattern = pattern_fields.iter()
                .map(|f| (f.name.clone(), f.field_type.clone(), f.is_required))
                .collect::<Vec<_>>();
            pattern.sort_by(|a, b| a.0.cmp(&b.0));
            
            let mut pattern_matched = false;
            
            // Check against each enum variant
            for (enum_variant_idx, enum_variant) in enum_info.variants.iter().enumerate() {
                if let Some(ref fields) = enum_variant.fields {
                    if self.verbose {
                        println!("                   🆚 vs enum variant '{}' ({})", enum_variant.name, enum_variant_idx + 1);
                    }
                    
                    let mut enum_pattern = fields.iter()
                        .map(|f| (f.name.clone(), f.field_type.clone(), !f.is_optional))
                        .collect::<Vec<_>>();
                    enum_pattern.sort_by(|a, b| a.0.cmp(&b.0));
                    
                    if self.verbose {
                        println!("                     Enum variant fields:");
                        for (name, field_type, required) in &enum_pattern {
                            println!("                       - {}: {} ({})", name, field_type, 
                                if *required { "required" } else { "optional" });
                        }
                    }
                    
                    if self.patterns_compatible(&pattern, &enum_pattern) {
                        if self.verbose {
                            println!("                     ✅ MATCH! Pattern {} matches enum variant '{}'", pattern_idx + 1, enum_variant.name);
                        }
                        matched_patterns += 1;
                        pattern_matched = true;
                        break;
                    } else if self.verbose {
                        println!("                     ❌ No match");
                    }
                } else if self.verbose {
                    println!("                   🆚 vs enum variant '{}' (unit variant) - skipping", enum_variant.name);
                }
            }
            
            if !pattern_matched && self.verbose {
                println!("                 ❌ Pattern {} did not match any enum variant", pattern_idx + 1);
            }
        }
        
        if total_patterns > 0 {
            let compatibility = matched_patterns as f64 / total_patterns as f64;
            if self.verbose {
                println!("               📊 Final compatibility: {}/{} = {:.1}%", 
                    matched_patterns, total_patterns, compatibility * 100.0);
            }
            Ok(Some(compatibility))
        } else {
            Ok(None)
        }
    }
    
    /// Try to fold back the variants into a struct with a field of existing enum type
    fn try_enum_fold_back(&self, common_fields: &[ShapeField], variants: &[ShapeVariant], base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        if self.verbose {
            println!("     🔍 Checking if variants match existing enum patterns...");
        }
        
        // For each known enum type, check if the variants match its structure
        for (type_name, type_info) in &self.known_types {
            if let TypeKind::Enum { info } = &type_info.kind {
                if self.verbose {
                    println!("       🔍 Checking against enum: {} (untagged: {})", type_name, info.is_untagged);
                }
                
                if let Some(fold_back) = self.check_enum_compatibility(common_fields, variants, type_name, info, base_name, original_shapes)? {
                    return Ok(Some(fold_back));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Check if the variants are compatible with a specific enum type
    fn check_enum_compatibility(&self, common_fields: &[ShapeField], variants: &[ShapeVariant], enum_name: &str, enum_info: &crate::parser::EnumInfo, base_name: &str, original_shapes: &[Shape]) -> Result<Option<EvolutionResult>> {
        // For untagged enums, we need to check if the variant field patterns match
        if !enum_info.is_untagged {
            // For tagged enums, this fold-back analysis doesn't apply
            return Ok(None);
        }
        
        if self.verbose {
            println!("         📊 Analyzing {} variants against {} enum variants", variants.len(), enum_info.variants.len());
        }
        
        // Create a mapping of field patterns to check compatibility
        let mut enum_patterns = Vec::new();
        for enum_variant in &enum_info.variants {
            if let Some(ref fields) = enum_variant.fields {
                let mut pattern = fields.iter()
                    .map(|f| (f.name.clone(), f.field_type.clone(), !f.is_optional))
                    .collect::<Vec<_>>();
                pattern.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by field name for consistent comparison
                enum_patterns.push(pattern);
            }
        }
        
        // Create patterns from our variants
        let mut our_patterns = Vec::new();
        for variant in variants {
            let mut pattern = variant.fields.iter()
                .map(|f| (f.name.clone(), f.field_type.clone(), f.is_required))
                .collect::<Vec<_>>();
            pattern.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by field name for consistent comparison
            our_patterns.push(pattern);
        }
        
        // Check if our patterns are a subset or match the enum patterns
        let mut matched_patterns = 0;
        for our_pattern in &our_patterns {
            for enum_pattern in &enum_patterns {
                if self.patterns_compatible(our_pattern, enum_pattern) {
                    matched_patterns += 1;
                    break;
                }
            }
        }

        // If we have common fields and ANY patterns match the enum, we can fold back to a struct
        // This represents an evolved struct with common fields + enum field
        if !common_fields.is_empty() && matched_patterns > 0 {
            if self.verbose {
                println!("         ✅ Found compatible enum! {} - {}/{} patterns matched", enum_name, matched_patterns, our_patterns.len());
                println!("         🔄 Folding back to evolved struct with common fields + {} field", enum_name);
            }
            
            // Create a struct with common fields plus a field of the enum type
            let mut final_fields = common_fields.to_vec();
            
            // Try to find the original enum field name from metadata
            let enum_field_name = if let Some(original_shape) = original_shapes.iter()
                .find(|shape| shape.metadata.source_enum_type.as_ref() == Some(&enum_name.to_string())) {
                if let Some(ref original_name) = original_shape.metadata.original_enum_field_name {
                    if self.verbose {
                        println!("         🔄 Using original enum field name from metadata: {}", original_name);
                    }
                    original_name.clone()
                } else {
                    if enum_name.to_lowercase().contains("variant") {
                        enum_name.trim_end_matches("Variant").to_lowercase()
                    } else {
                        format!("{}_type", enum_name.to_lowercase())
                    }
                }
            } else {
                if enum_name.to_lowercase().contains("variant") {
                    enum_name.trim_end_matches("Variant").to_lowercase()
                } else {
                    format!("{}_type", enum_name.to_lowercase())
                }
            };
            
            final_fields.push(ShapeField {
                name: enum_field_name,
                field_type: enum_name.to_string(),
                is_required: true,
            });

            // Check if we have unmatched patterns that need new enum variants
            if matched_patterns < our_patterns.len() {
                if self.verbose {
                    println!("         🔧 Creating new enum variants for {}/{} unmatched patterns", 
                        our_patterns.len() - matched_patterns, our_patterns.len());
                }
                
                // Create new variants for unmatched patterns
                let mut new_variants = Vec::new();
                for (i, our_pattern) in our_patterns.iter().enumerate() {
                    // Check if this pattern matched any existing enum variant
                    let mut pattern_matched = false;
                    for enum_pattern in &enum_patterns {
                        if self.patterns_compatible(our_pattern, enum_pattern) {
                            pattern_matched = true;
                            break;
                        }
                    }
                    
                    // If pattern didn't match, create a new variant
                    if !pattern_matched {
                        let variant_name = format!("NewVariant{}", i + 1);
                        let variant_fields: Vec<ShapeField> = our_pattern.iter()
                            .map(|(name, field_type, is_required)| ShapeField {
                                name: name.clone(),
                                field_type: field_type.clone(),
                                is_required: *is_required,
                            })
                            .collect();
                        
                        new_variants.push(ShapeVariant {
                            name: variant_name.clone(),
                            fields: variant_fields,
                        });
                        
                        if self.verbose {
                            println!("           ➕ Created new variant: {} with {} fields", variant_name, our_pattern.len());
                        }
                    }
                }
                
                if !new_variants.is_empty() {
                    if self.verbose {
                        println!("         📦 Returning struct with extended enum: {} new variants", new_variants.len());
                    }
                    return Ok(Some(EvolutionResult::struct_with_extended_enum(
                        base_name, 
                        final_fields, 
                        enum_name, 
                        new_variants
                    )));
                }
            }

            return Ok(Some(EvolutionResult::simple_struct(base_name, final_fields)));
        }

        // If most/all of our patterns match the enum (without common fields), we can still fold back
        let compatibility_threshold = (our_patterns.len() as f64 * 0.8).ceil() as usize; // 80% threshold
        if matched_patterns >= compatibility_threshold {
            if self.verbose {
                println!("         ✅ Found compatible enum! {} matched {} patterns", enum_name, matched_patterns);
                println!("         🔄 Folding back to struct with {} field of type {}", base_name, enum_name);
            }
            
            // Create a struct with common fields plus a field of the enum type
            let mut final_fields = common_fields.to_vec();
            
            // Add the enum field - try to detect a good name for it
            let enum_field_name = if enum_name.to_lowercase().contains("variant") {
                "variant".to_string()
            } else {
                format!("{}_type", enum_name.to_lowercase())
            };
            
            final_fields.push(ShapeField {
                name: enum_field_name,
                field_type: enum_name.to_string(),
                is_required: true,
            });

            return Ok(Some(EvolutionResult::simple_struct(base_name, final_fields)));
        }
        
        if self.verbose && matched_patterns > 0 {
            println!("         ⚠️  Partial match: {}/{} patterns matched (below {}% threshold)", 
                matched_patterns, our_patterns.len(), 80);
        }
        
        Ok(None)
    }
    
    /// Check if two field patterns are compatible
    fn patterns_compatible(&self, pattern1: &[(String, String, bool)], pattern2: &[(String, String, bool)]) -> bool {
        // For now, we require exact field name matches and compatible types
        if pattern1.len() != pattern2.len() {
            if self.verbose {
                println!("                       ❌ Field count mismatch: {} vs {}", pattern1.len(), pattern2.len());
            }
            return false;
        }
        
        for (field1, field2) in pattern1.iter().zip(pattern2.iter()) {
            // Field names must match
            if field1.0 != field2.0 {
                if self.verbose {
                    println!("                       ❌ Field name mismatch: '{}' vs '{}'", field1.0, field2.0);
                }
                return false;
            }
            
            // Types should be compatible (for now, we require exact match)
            if !self.types_compatible(&field1.1, &field2.1) {
                if self.verbose {
                    println!("                       ❌ Type mismatch for '{}': '{}' vs '{}'", field1.0, field1.1, field2.1);
                }
                return false;
            }
            
            // Required-ness should be compatible (required field can match optional, but not vice versa)
            if field2.2 && !field1.2 {
                if self.verbose {
                    println!("                       ❌ Required-ness mismatch for '{}': pattern has optional, enum requires required", field1.0);
                }
                return false;
            }
        }
        
        if self.verbose {
            println!("                       ✅ All fields compatible!");
        }
        true
    }
    
    /// Check if two types are compatible
    fn types_compatible(&self, type1: &str, type2: &str) -> bool {
        // Exact match
        if type1 == type2 {
            return true;
        }
        
        // Handle Option variations
        let clean_type1 = self.clean_type_string(type1);
        let clean_type2 = self.clean_type_string(type2);
        
        clean_type1 == clean_type2
    }
    
    /// Clean up type string for comparison
    fn clean_type_string(&self, type_str: &str) -> String {
        // Remove spaces around < and >
        type_str.replace(" < ", "<")
               .replace(" >", ">")
               .replace("< ", "<")
               .replace(" >", ">")
    }
    
    /// Create a mixed result that includes both folded-back patterns and non-matching variants
    fn create_mixed_fold_back_result(
        &self, 
        folded_variant_indices: &[usize], 
        non_matching_variant_indices: &[usize],
        common_field: &ShapeField,
        enum_type_name: &str,
        all_variants: &[ShapeVariant],
        base_name: &str,
        original_shapes: &[Shape]
    ) -> Result<Option<EvolutionResult>> {
        if self.verbose {
            println!("               🔧 Creating mixed fold-back result:");
            println!("                 - {} variants will be folded to use existing enum '{}'", folded_variant_indices.len(), enum_type_name);
            println!("                 - {} variants will remain as separate variants", non_matching_variant_indices.len());
        }
        
        let mut result_variants = Vec::new();
        
        // Create a variant for the folded-back pattern
        if !folded_variant_indices.is_empty() {
            // Try to find the original enum field name from any of the folded variants by looking at the original shapes
            if self.verbose {
                println!("                 🔍 Searching for original enum field name in {} original shapes", original_shapes.len());
                for (i, shape) in original_shapes.iter().enumerate() {
                    if let Some(ref field_name) = shape.metadata.original_enum_field_name {
                        println!("                   📋 Shape {}: has metadata with field_name='{}'", i, field_name);
                    } else {
                        println!("                   📋 Shape {}: no metadata field_name", i);
                    }
                }
            }
            
            let original_enum_field_name = original_shapes.iter()
                .find_map(|shape| shape.metadata.original_enum_field_name.clone());
            
            let enum_field_name = original_enum_field_name.clone().unwrap_or_else(|| {
                if enum_type_name.to_lowercase().contains("variant") {
                    enum_type_name.trim_end_matches("Variant").to_lowercase()
                } else {
                    format!("{}_type", enum_type_name.to_lowercase())
                }
            });
            
            if self.verbose {
                match &original_enum_field_name {
                    Some(name) => println!("                 🔄 Using original field name from metadata: {}", name),
                    None => println!("                 🔄 Using derived field name: {}", enum_field_name),
                }
            }
            
            let folded_variant = ShapeVariant {
                name: format!("{}Pattern", enum_type_name.trim_end_matches("Variant")),
                fields: vec![
                    common_field.clone(),
                    ShapeField {
                        name: enum_field_name,
                        field_type: enum_type_name.to_string(),
                        is_required: true,
                    }
                ]
            };
            
            if self.verbose {
                println!("                 ✅ Created folded variant: {}", folded_variant.name);
            }
            
            result_variants.push(folded_variant);
        }
        
        // Add the non-matching variants as-is
        for &idx in non_matching_variant_indices {
            let variant = &all_variants[idx];
            if self.verbose {
                println!("                 ✅ Added non-matching variant: {}", variant.name);
            }
            result_variants.push(variant.clone());
        }
        
        if self.verbose {
            println!("               🎯 Final mixed result: {} total variants", result_variants.len());
        }
        
        // For now, create a complex enum with no common fields
        // TODO: Could be enhanced to detect common fields across the mixed result
        Ok(Some(EvolutionResult::complex_enum(base_name, vec![], result_variants)))
    }
    
    /// Choose the better field type between two fields with the same name
    /// Prioritizes existing types over JSON-inferred types
    fn choose_better_field_type(&self, field1: &ShapeField, field2: &ShapeField) -> ShapeField {
        // If types are the same, return field1
        if field1.field_type == field2.field_type {
            return field1.clone();
        }
        
        // Prioritize more specific integer types over generic i64/u64
        // This helps preserve existing type precision
        match (&field1.field_type[..], &field2.field_type[..]) {
            // i32 wins over i64 (existing type precision)
            ("i32", "i64") => field1.clone(),
            ("i64", "i32") => field2.clone(),
            
            // i16 wins over i32/i64 
            ("i16", "i32" | "i64") => field1.clone(),
            ("i32" | "i64", "i16") => field2.clone(),
            
            // u32 wins over u64
            ("u32", "u64") => field1.clone(),
            ("u64", "u32") => field2.clone(),
            
            // u16 wins over u32/u64
            ("u16", "u32" | "u64") => field1.clone(),
            ("u32" | "u64", "u16") => field2.clone(),
            
            // If one field is required and other is optional, prefer required
            _ if field1.is_required && !field2.is_required => field1.clone(),
            _ if !field1.is_required && field2.is_required => field2.clone(),
            
            // Default: return field1 (first encountered)
            _ => field1.clone(),
        }
    }
}
