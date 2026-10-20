// The sea's vertex shader: where the grid becomes water.
//
// Bevy's own mesh vertex shader reads a position out of a buffer and transforms it.
// This one reads a *grid offset* out of the same slot and works out, from that offset
// alone, where that patch of water is, which way it faces, what colour it is and where
// it reads the ripple map. Everything else in the pass — lighting, shadows, the normal
// map, the environment reflection — is the standard material's, untouched.
//
// The mesh is therefore built once at startup and never written again. What used to be
// forty thousand vertices copied across the bus every frame is now one eighty-kilobyte
// buffer of this frame's numbers.

import bevy_pbr::render::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput, decompress_vertex},
    view_transformations::position_world_to_clip,
};
import super::sea_wave::dress;

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(in, in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);

    // The position attribute is the grid offset from the boat, in the mesh's own frame:
    // x east-ish, z towards the viewer. It carries no height at all — the height is the
    // thing this shader is for.
    let water = dress(vec2<f32>(vertex.position.x, vertex.position.z));

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(water.position, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);

    // Analytic normals, from the field's own slope rather than from the triangles: both
    // cheaper than recomputing them and exactly right, and it means the lighting shows
    // the wave shape even where the mesh is too coarse to.
@if(VERTEX_NORMALS)
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        water.normal,
        in.instance_index,
    );

@if(VERTEX_TANGENTS)
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        water.tangent,
        in.instance_index,
    );

@if(VERTEX_UVS_A)
    out.uv = water.uv;

@if(VERTEX_COLORS)
    out.color = water.colour;

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        in.instance_index,
        world_from_local[3],
    );

    return out;
}
