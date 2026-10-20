// The same water, for the passes that only want to know where it is.
//
// **A displaced surface has to be displaced in every pass or it is two surfaces.** The
// shadow map is drawn with the prepass pipeline, and a prepass that read the flat grid
// would have the sun casting the shadow of a plane at mean sea level: every trough in
// the picture dark, because every trough is below it. That is the whole reason this
// file exists, and it is why it computes the position the same way `sea.wesl` does —
// by calling the same function, not by repeating it.
//
// It costs four more passes over the grid, which on a card drawing a 960-pixel world is
// nothing, and it keeps the waves shadowing each other in a low sun.

import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{
        mesh_functions,
        view_transformations::position_world_to_clip,
    },
};
import super::sea_wave::dress;

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let vertex = decompress_vertex(in, in.instance_index);
    let world_from_local = mesh_functions::get_world_from_local(in.instance_index);

    let water = dress(vec2<f32>(vertex.position.x, vertex.position.z));

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(water.position, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);

@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(VERTEX_UVS_A)
    out.uv = water.uv;

@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
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
}

@if(VERTEX_COLORS)
    out.color = water.colour;

    // Where this patch of water was last frame, for anything that wants motion. The sea
    // has no previous answer to give — this shader only knows about now — so it hands
    // back where the water is, which says the surface did not move. That is wrong for a
    // wave and right for everything the prepass does with it here: nothing in this game
    // runs a motion-vector pass, and a sea that reported its orbital velocity to a
    // temporal filter would smear the picture rather than sharpen it.
@if(MOTION_VECTOR_PREPASS)
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(
        mesh_functions::get_previous_world_from_local(in.instance_index),
        vec4<f32>(water.position, 1.0),
    );

@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = in.instance_index;

@if(VISIBILITY_RANGE_DITHER)
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        in.instance_index,
        world_from_local[3],
    );

    return out;
}
