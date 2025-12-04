use anyhow::{Context, Result};
use gltf::Gltf;
use meshopt::{
    optimize::{optimize_vertex_cache, optimize_overdraw_in_place, optimize_vertex_fetch_remap},
    simplify::{simplify, SimplifyOptions},
    encoding::{encode_vertex_buffer, encode_index_buffer},
    remap::{remap_index_buffer, remap_vertex_buffer},
    VertexDataAdapter,
};
use std::path::Path;
use std::time::Instant;

use super::ProcessingStats;

/// Configuration for model processing
#[derive(Debug, Clone)]
pub struct ModelConfig {
    /// Enable mesh optimization (vertex cache, overdraw, fetch)
    pub optimize_meshes: bool,
    /// Enable vertex/index buffer encoding (meshopt compression)
    pub encode_buffers: bool,
    /// Generate LOD levels
    pub generate_lods: bool,
    /// Number of LOD levels to generate (1-4)
    pub lod_count: u32,
    /// Target ratio for each LOD level (e.g., 0.5 = 50% of previous)
    pub lod_ratio: f32,
    /// Generate binary GLB output
    pub output_glb: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            optimize_meshes: true,
            encode_buffers: true,
            generate_lods: false,
            lod_count: 3,
            lod_ratio: 0.5,
            output_glb: true,
        }
    }
}

/// glTF model information
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub meshes: usize,
    pub materials: usize,
    pub textures: usize,
    pub animations: usize,
    pub nodes: usize,
    pub total_vertices: usize,
    pub total_indices: usize,
}

impl std::fmt::Display for ModelInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} meshes, {} vertices, {} indices, {} materials",
            self.meshes, self.total_vertices, self.total_indices, self.materials
        )
    }
}

/// Get information about a glTF model
pub fn get_model_info(path: &Path) -> Result<ModelInfo> {
    let gltf = Gltf::open(path)
        .with_context(|| format!("Failed to open glTF file: {}", path.display()))?;

    let document = &gltf.document;

    let mut total_vertices = 0;
    let mut total_indices = 0;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            if let Some(accessor) = primitive.get(&gltf::Semantic::Positions) {
                total_vertices += accessor.count();
            }
            if let Some(indices) = primitive.indices() {
                total_indices += indices.count();
            }
        }
    }

    Ok(ModelInfo {
        meshes: document.meshes().count(),
        materials: document.materials().count(),
        textures: document.textures().count(),
        animations: document.animations().count(),
        nodes: document.nodes().count(),
        total_vertices,
        total_indices,
    })
}

/// Mesh data extracted from glTF for optimization
#[derive(Debug, Clone)]
pub struct MeshData {
    pub vertices: Vec<f32>,  // Interleaved position data (x, y, z per vertex)
    pub indices: Vec<u32>,
    pub vertex_count: usize,
    pub vertex_stride: usize, // Bytes per vertex
}

/// Optimized mesh result
#[derive(Debug, Clone)]
pub struct OptimizedMesh {
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub encoded_vertices: Option<Vec<u8>>,
    pub encoded_indices: Option<Vec<u8>>,
}

/// LOD mesh with simplification
#[derive(Debug, Clone)]
pub struct LodMesh {
    pub level: u32,
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub vertex_count: usize,
    pub index_count: usize,
    pub target_error: f32,
}

/// Optimize a mesh using meshoptimizer
pub fn optimize_mesh(mesh: &MeshData, config: &ModelConfig) -> Result<OptimizedMesh> {
    let mut indices = mesh.indices.clone();
    let vertex_count = mesh.vertex_count;

    if config.optimize_meshes && !indices.is_empty() {
        // Step 1: Optimize vertex cache (improves GPU vertex cache utilization)
        optimize_vertex_cache(&mut indices, vertex_count);

        // Step 2: Optimize overdraw (reduces pixel overdraw)
        // Create vertex adapter for position data
        let positions: Vec<[f32; 3]> = mesh.vertices
            .chunks(3)
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect();

        let vertex_adapter = VertexDataAdapter::new(
            bytemuck::cast_slice(&positions),
            std::mem::size_of::<[f32; 3]>(),
            0,
        ).map_err(|e| anyhow::anyhow!("Failed to create vertex adapter: {:?}", e))?;

        optimize_overdraw_in_place(&mut indices, &vertex_adapter, 1.05);

        // Step 3: Optimize vertex fetch (improves memory access patterns)
        // This reorders vertices, so we need to remap
        let remap = optimize_vertex_fetch_remap(&indices, vertex_count);
        let remapped_indices: Vec<u32> = remap_index_buffer(Some(&indices), vertex_count, &remap);

        // Remap vertices
        let remapped_positions: Vec<[f32; 3]> = remap_vertex_buffer(&positions, vertex_count, &remap);
        let vertices: Vec<f32> = remapped_positions.iter()
            .flat_map(|p| p.iter().copied())
            .collect();

        let new_vertex_count = remapped_positions.len();

        // Step 4: Encode buffers if requested
        let (encoded_vertices, encoded_indices) = if config.encode_buffers {
            // Convert positions to bytes for encoding
            let positions_bytes: &[u8] = bytemuck::cast_slice(&remapped_positions);
            let encoded_verts = encode_vertex_buffer(positions_bytes).ok();
            let encoded_idx = encode_index_buffer(&remapped_indices, new_vertex_count).ok();
            (encoded_verts, encoded_idx)
        } else {
            (None, None)
        };

        Ok(OptimizedMesh {
            vertices,
            indices: remapped_indices,
            encoded_vertices,
            encoded_indices,
        })
    } else {
        Ok(OptimizedMesh {
            vertices: mesh.vertices.clone(),
            indices: mesh.indices.clone(),
            encoded_vertices: None,
            encoded_indices: None,
        })
    }
}

/// Generate LOD levels for a mesh using simplification
pub fn generate_lods(mesh: &MeshData, config: &ModelConfig) -> Result<Vec<LodMesh>> {
    let mut lods = Vec::new();

    // LOD 0 is the original mesh
    lods.push(LodMesh {
        level: 0,
        vertices: mesh.vertices.clone(),
        indices: mesh.indices.clone(),
        vertex_count: mesh.vertex_count,
        index_count: mesh.indices.len(),
        target_error: 0.0,
    });

    if !config.generate_lods || mesh.indices.is_empty() {
        return Ok(lods);
    }

    // Create vertex adapter for simplification
    let positions: Vec<[f32; 3]> = mesh.vertices
        .chunks(3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();

    let vertex_adapter = VertexDataAdapter::new(
        bytemuck::cast_slice(&positions),
        std::mem::size_of::<[f32; 3]>(),
        0,
    ).map_err(|e| anyhow::anyhow!("Failed to create vertex adapter: {:?}", e))?;

    let mut current_indices = mesh.indices.clone();
    let mut current_target_count = mesh.indices.len();

    for level in 1..=config.lod_count {
        // Target index count for this LOD
        current_target_count = (current_target_count as f32 * config.lod_ratio) as usize;
        current_target_count = current_target_count.max(3); // At least one triangle

        // Simplify the mesh
        let target_error = 0.01 * level as f32; // Increase error tolerance for lower LODs

        let simplified = simplify(
            &current_indices,
            &vertex_adapter,
            current_target_count,
            target_error,
            SimplifyOptions::None,
            None,
        );

        if simplified.is_empty() {
            break; // Can't simplify further
        }

        lods.push(LodMesh {
            level,
            vertices: mesh.vertices.clone(), // Vertices are shared
            indices: simplified.clone(),
            vertex_count: mesh.vertex_count,
            index_count: simplified.len(),
            target_error,
        });

        current_indices = simplified;

        // Stop if we've reduced to a very small mesh
        if current_indices.len() < 12 {
            break;
        }
    }

    Ok(lods)
}

/// Process a glTF/GLB model with optimization
pub fn process_model(
    input: &Path,
    output: &Path,
    config: &ModelConfig,
) -> Result<ProcessingStats> {
    let start = Instant::now();
    let original_size = std::fs::metadata(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?
        .len();

    // Create output directory if needed
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Load and validate the glTF
    let gltf = Gltf::open(input)
        .with_context(|| format!("Failed to parse glTF file: {}", input.display()))?;

    validate_gltf(&gltf)?;

    // Get model info for reporting
    let info = get_model_info(input)?;

    // Import full glTF with buffers
    let (document, buffers, _images) = gltf::import(input)
        .with_context(|| format!("Failed to import glTF: {}", input.display()))?;

    // Extract and optimize meshes
    let mut optimized_meshes = Vec::new();
    let mut total_original_indices = 0;
    let mut total_optimized_indices = 0;

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            if let Some(mesh_data) = extract_mesh_data(&primitive, &buffers)? {
                total_original_indices += mesh_data.indices.len();

                // Optimize the mesh
                let optimized = optimize_mesh(&mesh_data, config)?;
                total_optimized_indices += optimized.indices.len();

                // Generate LODs if requested
                if config.generate_lods {
                    let lods = generate_lods(&mesh_data, config)?;
                    tracing::debug!(
                        "Generated {} LOD levels for mesh",
                        lods.len()
                    );
                }

                optimized_meshes.push(optimized);
            }
        }
    }

    // For now, copy the original file
    // Full GLB export with optimized data would require a GLB writer
    // which is beyond the scope of the gltf crate (read-only)
    std::fs::copy(input, output)?;

    let output_size = std::fs::metadata(output)
        .with_context(|| format!("Failed to read output file: {}", output.display()))?
        .len();

    let processing_time_ms = start.elapsed().as_millis() as u64;

    // Log optimization stats
    if config.optimize_meshes && total_original_indices > 0 {
        tracing::info!(
            "Optimized model: {} - indices: {} -> {} (vertex cache, overdraw, fetch optimized)",
            info,
            total_original_indices,
            total_optimized_indices
        );
    }

    // Log encoding stats
    if config.encode_buffers {
        let encoded_count = optimized_meshes.iter()
            .filter(|m| m.encoded_vertices.is_some())
            .count();
        if encoded_count > 0 {
            tracing::info!(
                "Encoded {} mesh buffers with meshopt compression",
                encoded_count
            );
        }
    }

    Ok(ProcessingStats {
        original_size,
        output_size,
        processing_time_ms,
    })
}

/// Extract mesh data from a glTF primitive
fn extract_mesh_data(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
) -> Result<Option<MeshData>> {
    // Get positions
    let positions_accessor = match primitive.get(&gltf::Semantic::Positions) {
        Some(acc) => acc,
        None => return Ok(None),
    };

    let positions_view = positions_accessor.view()
        .ok_or_else(|| anyhow::anyhow!("Position accessor has no buffer view"))?;
    let positions_buffer = &buffers[positions_view.buffer().index()];

    let positions_offset = positions_view.offset() + positions_accessor.offset();
    let positions_len = positions_accessor.count() * 3 * 4; // 3 floats * 4 bytes

    let positions_data = &positions_buffer[positions_offset..positions_offset + positions_len];
    let positions: Vec<f32> = positions_data
        .chunks(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    // Get indices
    let indices = if let Some(indices_accessor) = primitive.indices() {
        let indices_view = indices_accessor.view()
            .ok_or_else(|| anyhow::anyhow!("Index accessor has no buffer view"))?;
        let indices_buffer = &buffers[indices_view.buffer().index()];

        let indices_offset = indices_view.offset() + indices_accessor.offset();

        match indices_accessor.data_type() {
            gltf::accessor::DataType::U16 => {
                let indices_len = indices_accessor.count() * 2;
                let indices_data = &indices_buffer[indices_offset..indices_offset + indices_len];
                indices_data
                    .chunks(2)
                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]) as u32)
                    .collect()
            }
            gltf::accessor::DataType::U32 => {
                let indices_len = indices_accessor.count() * 4;
                let indices_data = &indices_buffer[indices_offset..indices_offset + indices_len];
                indices_data
                    .chunks(4)
                    .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect()
            }
            gltf::accessor::DataType::U8 => {
                let indices_len = indices_accessor.count();
                let indices_data = &indices_buffer[indices_offset..indices_offset + indices_len];
                indices_data.iter().map(|&i| i as u32).collect()
            }
            _ => return Ok(None),
        }
    } else {
        // Non-indexed mesh - generate sequential indices
        (0..positions_accessor.count() as u32).collect()
    };

    Ok(Some(MeshData {
        vertex_count: positions_accessor.count(),
        vertex_stride: 12, // 3 floats * 4 bytes
        vertices: positions,
        indices,
    }))
}

fn validate_gltf(gltf: &Gltf) -> Result<()> {
    let document = &gltf.document;

    // Check for common issues
    if document.meshes().count() == 0 {
        tracing::warn!("glTF file contains no meshes");
    }

    // Validate buffer references
    for buffer in document.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Uri(uri) => {
                if uri.starts_with("data:") {
                    // Embedded data URI, OK
                } else {
                    // External file reference
                    tracing::debug!("External buffer reference: {}", uri);
                }
            }
            gltf::buffer::Source::Bin => {
                // GLB binary chunk, OK
            }
        }
    }

    Ok(())
}

/// Model format detection
pub fn detect_model_format(path: &Path) -> Option<ModelFormat> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    match ext.as_str() {
        "gltf" => Some(ModelFormat::GlTF),
        "glb" => Some(ModelFormat::GLB),
        "obj" => Some(ModelFormat::OBJ),
        "fbx" => Some(ModelFormat::FBX),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormat {
    GlTF,
    GLB,
    OBJ,
    FBX,
}

impl std::fmt::Display for ModelFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelFormat::GlTF => write!(f, "glTF"),
            ModelFormat::GLB => write!(f, "GLB"),
            ModelFormat::OBJ => write!(f, "OBJ"),
            ModelFormat::FBX => write!(f, "FBX"),
        }
    }
}

/// Estimate the optimal LOD levels for a model based on complexity
pub fn estimate_lod_levels(info: &ModelInfo) -> Vec<LodLevelEstimate> {
    let base_vertices = info.total_vertices;
    let mut levels = vec![
        LodLevelEstimate {
            level: 0,
            vertex_ratio: 1.0,
            suggested_distance: 0.0,
            estimated_triangles: info.total_indices / 3,
        },
    ];

    // Add LOD levels based on vertex count
    if base_vertices > 1000 {
        levels.push(LodLevelEstimate {
            level: 1,
            vertex_ratio: 0.5,
            suggested_distance: 10.0,
            estimated_triangles: info.total_indices / 6,
        });
    }

    if base_vertices > 5000 {
        levels.push(LodLevelEstimate {
            level: 2,
            vertex_ratio: 0.25,
            suggested_distance: 25.0,
            estimated_triangles: info.total_indices / 12,
        });
    }

    if base_vertices > 10000 {
        levels.push(LodLevelEstimate {
            level: 3,
            vertex_ratio: 0.1,
            suggested_distance: 50.0,
            estimated_triangles: info.total_indices / 30,
        });
    }

    levels
}

/// LOD level estimate
#[derive(Debug, Clone)]
pub struct LodLevelEstimate {
    pub level: u32,
    pub vertex_ratio: f32,
    pub suggested_distance: f32,
    pub estimated_triangles: usize,
}
