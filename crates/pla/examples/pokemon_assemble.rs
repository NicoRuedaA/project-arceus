use pla::assets::pokemon_model::{
    assemble_pack, write_skinned_lod_level, write_static_lod_level, write_static_lod_level_baked,
    LayerBlendFormula,
};
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let pack = PathBuf::from(
        args.next()
            .ok_or("usage: pokemon_assemble <pack.gfpak> <output-dir>")?,
    );
    let out = PathBuf::from(
        args.next()
            .ok_or("usage: pokemon_assemble <pack.gfpak> <output-dir>")?,
    );
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let model = assemble_pack(&pack)?;
    let mut report = String::new();
    report.push_str(&format!(
        "pack={}\nentries={}\n",
        model.pack_path.display(),
        model.entry_count
    ));
    report.push_str(&format!("meshes={} skeleton={} bind_records={} consumed_bind_records={} materials={} textures={} parsed_clips={}\n",
        model.meshes.len(), model.skeleton.is_some(), model.bind_record_count.map_or_else(|| "unknown".into(), |n| n.to_string()), model.bind_records.len(), model.materials.len(), model.textures.len(),
        model.clips.iter().filter(|clip| clip.parsed.is_ok()).count()));
    if let Some(skeleton) = &model.skeleton {
        report.push_str(&format!(
            "skeleton_entry={} nodes={} declared_bind_count={} bind_error={}\n",
            model.skeleton_entry_index.unwrap_or(usize::MAX),
            skeleton.nodes.len(),
            skeleton.bind_count,
            model.bind_record_error.as_deref().unwrap_or("none")
        ));
    }
    if let Some(error) = &model.skeleton_parse_error {
        report.push_str(&format!("skeleton_parse_error={error}\n"));
    }
    report.push_str(&format!(
        "skeleton_nodes={} node_type_counts={:?}\n",
        model
            .skeleton_node_count
            .map_or_else(|| "unknown".into(), |n| n.to_string()),
        model.skeleton_node_type_counts
    ));
    for mesh in &model.meshes {
        report.push_str(&format!("shape={} shape_index={} lod={} mesh_entry={} buffer_entry={} vertices={} indices={} material_entry={}\n",
            mesh.name, mesh.shape_index, mesh.lod.map_or_else(|| "unknown".into(), |lod| lod.to_string()), mesh.entry_index,
            mesh.buffer_entry_index, mesh.mesh.vertex_count, mesh.mesh.indices.len(),
            mesh.material_entry_index.map_or_else(|| "unknown".into(), |i| i.to_string())));
        if let Some(layout) = &mesh.mesh.layout {
            let attributes = layout
                .attributes
                .iter()
                .map(|attribute| {
                    (
                        attribute.id,
                        attribute.format_code,
                        attribute.offset,
                        attribute.byte_size,
                    )
                })
                .collect::<Vec<_>>();
            report.push_str(&format!(
                "shape_layout={} stride={} attributes={attributes:?}\n",
                mesh.name, layout.stride
            ));
        }
        let mut uv_min = [f32::INFINITY; 2];
        let mut uv_max = [f32::NEG_INFINITY; 2];
        let uv_count = (0..mesh.mesh.vertex_count)
            .filter_map(|vertex| mesh.mesh.uv(&mesh.buffer_bytes, vertex))
            .inspect(|uv| {
                for axis in 0..2 {
                    uv_min[axis] = uv_min[axis].min(uv[axis]);
                    uv_max[axis] = uv_max[axis].max(uv[axis]);
                }
            })
            .count();
        report.push_str(&format!(
            "shape_uv={} count={} min={uv_min:?} max={uv_max:?}\n",
            mesh.name, uv_count
        ));
    }
    for lod in 0..=2 {
        match write_static_lod_level(&model, lod, out.join(format!("pikachu_lod{lod}.gltf"))) {
            Ok(()) => report.push_str(&format!("static_lod{lod}_export=ok\n")),
            Err(error) => report.push_str(&format!("static_lod{lod}_export=failed: {error}\n")),
        }
    }
    for (name, formula) in [
        ("weighted_sum", LayerBlendFormula::WeightedSum),
        ("sequential_lerp", LayerBlendFormula::SequentialLerp),
    ] {
        let output = out.join(format!("pikachu_lod0_{name}.gltf"));
        match write_static_lod_level_baked(&model, 0, &output, formula) {
            Ok(()) => report.push_str(&format!("layer_bake_{name}=ok\n")),
            Err(error) => report.push_str(&format!("layer_bake_{name}=blocked:{error}\n")),
        }
    }
    for texture in &model.textures {
        if let Ok(image) = &texture.image {
            let filename = format!("{}.png", texture.internal_name.replace('/', "_"));
            if let Err(error) = image.save_png(&out.join(filename)) {
                report.push_str(&format!(
                    "texture_export={}:failed:{error}\n",
                    texture.internal_name
                ));
            }
        }
    }
    for material in &model.materials {
        report.push_str(&format!(
            "material_entry={} texture_refs={} resolved_links={}\n",
            material.entry_index,
            material.parsed.textures.len(),
            material.texture_links.len()
        ));
        let links = material
            .texture_links
            .iter()
            .map(|(role, name)| format!("{role}:{name}"))
            .collect::<Vec<_>>()
            .join(",");
        report.push_str(&format!(
            "material_links={links} flags={:?}\n",
            material.parsed.flags
        ));
    }
    for clip in &model.clips {
        report.push_str(&format!(
            "clip={} entry={} parse_result={} companion={}\n",
            clip.name,
            clip.animation_entry_index,
            clip.parsed
                .as_ref()
                .map(|_| "ok".to_string())
                .unwrap_or_else(|e| format!("failed:{e}")),
            clip.companion_entry_index
                .map_or_else(|| "unknown".into(), |i| i.to_string())
        ));
    }
    for wanted in ["defaultwait01_loop", "walk01_loop"] {
        let matching = model.clips.iter().find(|clip| clip.name.ends_with(wanted));
        match matching {
            Some(clip) => {
                let output = out.join(format!("pikachu_{wanted}.gltf"));
                match write_skinned_lod_level(
                    &model,
                    0,
                    &clip.name,
                    &output,
                    Some(LayerBlendFormula::WeightedSum),
                ) {
                    Ok(()) => report.push_str(&format!("skinned_export={wanted}:ok\n")),
                    Err(error) => {
                        report.push_str(&format!("skinned_export={wanted}:blocked: {error}\n"))
                    }
                }
            }
            None => report.push_str(&format!("skinned_export={wanted}:clip_not_found\n")),
        }
    }
    for diagnostic in &model.diagnostics {
        report.push_str(&format!("diagnostic={diagnostic}\n"));
    }
    fs::write(out.join("assembly.txt"), report).map_err(|e| e.to_string())?;
    Ok(())
}
