//! Ink outlines, the way a cel-shaded game draws them.
//!
//! Two marks, because an inker makes two marks and they are not the same mark.
//!
//! The **silhouette** is the heavy line around the outside of a shape, and it is drawn
//! by the oldest trick in the trade: build a second copy of the hull, a little larger,
//! turned inside out, and paint it flat black. Only the far side of that copy survives
//! the back-face cull, so it is hidden everywhere the real object covers it and shows
//! only as a rim where the object ends. No shader, no second render pass, no edge
//! detection — one extra mesh and a culling mode.
//!
//! The **creases** are the lighter lines inside the shape: the line along a chine, the
//! line where the coaming meets the deck, the line round a hatch. Those are not
//! silhouettes from any angle, so the shell cannot draw them. They come from the mesh's
//! own topology instead: an edge shared by two faces that disagree sharply about which
//! way is out is a fold, and a fold is what an inker draws. They are laid down as line
//! primitives, which land one pixel wide no matter how far off they are — which is
//! exactly right, because a drawn line does not get thinner with distance either.
//!
//! Both have to weld the mesh first, and that is the part that is easy to get wrong.
//! A mesh that has been through a modelling tool carries split vertices wherever a
//! texture seam or a hard normal runs: the same corner of the same triangle appears two
//! or three times at identical coordinates with different normals. Push those apart
//! along their own normals and the shell tears open along every seam; test their edges
//! for folds and every seam reads as a fold, so the model comes back wearing a net.
//! Welding by position first — millimetre buckets — makes the surface one surface
//! again, which is what both marks are asking about.

use std::collections::HashMap;

/// What meets along one welded edge: the first face's normal, the second's if there is
/// one, whether a third turned up, and the two original vertices to draw between.
type EdgeFaces = (Vec3, Option<Vec3>, bool, [usize; 2]);

/// Mesh entities eligible for ink: anything with geometry that is not already an
/// outline, not already done, and not a sail or a telltale — cloth gets no ink line,
/// because cloth has no edge, and a telltale is three centimetres of wool.
type InkSources<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Mesh3d,
        Option<&'static MeshMaterial3d<StandardMaterial>>,
    ),
    (
        Without<Outline>,
        Without<Outlined>,
        Without<SailSurface>,
        Without<Telltale>,
    ),
>;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;

use crate::boat::{SailSurface, Telltale};
use crate::world::Ink;

/// Put this on anything whose descendants should be inked.
///
/// On the boat's root rather than on each of its pieces, because by the time the hull
/// scan has finished loading its meshes are somewhere down a hierarchy this crate never
/// built and cannot name.
#[derive(Component)]
pub struct Inked;

/// An outline, so it can be found again and taken off.
#[derive(Component)]
pub struct Outline;

/// Set on a mesh entity once its outline exists, so the work happens once.
#[derive(Component)]
pub struct Outlined;

/// How wide the silhouette is drawn, as a fraction of the object's own size.
///
/// Relative rather than absolute because the same scene holds an eleven-metre boat and
/// a four-hundred-metre island. A fixed offset that reads on the boat is invisible on
/// the island, and one that reads on the island swallows the boat. A comic inks the
/// large shape with the bolder line anyway.
///
/// Thinned twice: 0.0045 to 0.0028 and then to 0.0012, which is a hairline. The whole
/// weight of the drawing moves to the *number* of lines rather than their thickness —
/// a fine pen used everywhere, which is the manga hand rather than the comic-book one.
/// The mast now carries 18 mm, the hull 14, a winch 5.
///
/// Thinned from 0.0045, because the boldest lines in the picture — the mast and the
/// topsides, which are also the longest objects — were the ones overdoing it: 69 mm and
/// 52 mm of black around a spar a hundred millimetres thick and a hull you are standing
/// on. They draw 43 mm and 32 mm now, the boom 17, a winch 10, and an island still
/// reaches its cap. A line should say where a thing ends, not become the thing.
const SILHOUETTE: f32 = 0.0012;

/// Absolute bounds on it, because the relative rule has to be stopped at both ends: a
/// winch handle would get a line too fine to see, an island one you could moor to.
const SILHOUETTE_MIN: f32 = 0.005;
const SILHOUETTE_MAX: f32 = 0.9;

/// How sharply two faces must disagree before the edge between them is a fold.
///
/// Thirty-two degrees, down from sixty, which draws far more of the boat: every box
/// corner, every change of plane in the deck furniture, the chamfer on a hatch. With a
/// hairline to draw them in, more lines is what adds the detail that thickness used to —
/// and the share rule below is what keeps the extra sensitivity from turning a scan's
/// grain into a net.
///
/// Sixty degrees, which is generous, and on its own nowhere near enough. Measured on
/// this boat: at sixty, the hull scan called 23 187 of its 41 500 edges folds — 56% of
/// the mesh — while the islands, which are drawn by a formula, called 5 to 9%. The
/// scan is not a boat with a great many chines in it. It is a surface with grain, and
/// the angle test cannot tell grain from a fold because at the scale of one triangle
/// there is no difference. See `CHAINED` for what does tell them apart.
const CREASE_ANGLE: f32 = 32.0;

/// The share of a mesh's edges that may fold before the mesh is judged to have no folds
/// in it at all.
///
/// This replaced a cleverer rule that did not work, and the way it failed is worth
/// keeping. The idea was that a crease you would draw *runs* — a chine runs the length
/// of the boat, grain in a scan tilts one facet and dead-ends — so an edge would be
/// kept only if both its ends carried another fold. It cost one pass and it was wrong:
/// measured, it took the scan from 23 187 lines to 23 154, which is a tenth of a per
/// cent. Grain chains too. A tilted facet's four edges each meet the next facet's, and
/// the speckle is a connected web rather than a scatter of dashes.
///
/// What does separate them is the share, and the numbers leave a wide gap to put a
/// threshold in: the islands, which are drawn by a formula and have real folds where
/// the bank meets the shelf, fold 5 to 9% of their edges. The hull scan folds 56%. A
/// surface that disagrees with itself across more than a quarter of its edges is not a
/// shape with creases in it, it is a shape with grain, and the honest thing is to draw
/// its silhouette and leave its interior alone. A scan is a measurement, not a drawing;
/// it has no lines in it to find.
const GRAIN_SHARE: f32 = 0.25;

/// How far the crease lines are lifted off the surface they belong to.
///
/// A line lying exactly in the surface fights the surface for the depth buffer and the
/// result is a dashed line that reshuffles its dashes every time the camera moves.
/// Lifting it along the welded normal by a couple of millimetres settles it.
const CREASE_LIFT: f32 = 0.004;

fn positions(mesh: &Mesh) -> Option<&Vec<[f32; 3]>> {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => Some(v),
        _ => None,
    }
}

fn normals(mesh: &Mesh) -> Option<&Vec<[f32; 3]>> {
    match mesh.attribute(Mesh::ATTRIBUTE_NORMAL)? {
        VertexAttributeValues::Float32x3(v) => Some(v),
        _ => None,
    }
}

fn triangles(mesh: &Mesh) -> Option<Vec<[usize; 3]>> {
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let idx: Vec<usize> = match mesh.indices()? {
        Indices::U16(v) => v.iter().map(|&i| i as usize).collect(),
        Indices::U32(v) => v.iter().map(|&i| i as usize).collect(),
    };
    Some(
        idx.as_chunks::<3>()
            .0
            .iter()
            .map(|c| [c[0], c[1], c[2]])
            .collect(),
    )
}

/// Weld by position: for each vertex, the index of the first vertex sharing its corner.
///
/// Millimetre buckets. Finer and a scan's own coordinate noise leaves seams unwelded;
/// coarser and genuinely separate detail gets fused.
fn weld(pos: &[[f32; 3]]) -> Vec<usize> {
    let mut first: HashMap<(i32, i32, i32), usize> = HashMap::with_capacity(pos.len());
    let key = |p: &[f32; 3]| {
        (
            (p[0] * 1000.0).round() as i32,
            (p[1] * 1000.0).round() as i32,
            (p[2] * 1000.0).round() as i32,
        )
    };
    pos.iter()
        .enumerate()
        .map(|(i, p)| *first.entry(key(p)).or_insert(i))
        .collect()
}

/// The direction "out" at each corner, averaged across every vertex sharing it.
fn smooth_normals(pos: &[[f32; 3]], norm: &[[f32; 3]], welded: &[usize]) -> Vec<Vec3> {
    let mut sum = vec![Vec3::ZERO; pos.len()];
    for (i, n) in norm.iter().enumerate() {
        sum[welded[i]] += Vec3::from_array(*n);
    }
    (0..pos.len())
        .map(|i| {
            sum[welded[i]]
                .try_normalize()
                .unwrap_or_else(|| Vec3::from_array(norm[i]))
        })
        .collect()
}

/// The inside-out copy that draws the silhouette.
///
/// The winding is left alone and the material culls front faces instead, which is the
/// same thing said once rather than twice: reversing the indices *and* culling would
/// cancel out, and a sign error here shows up as an object with no outline at all,
/// which looks exactly like the system not having run.
pub fn shell(mesh: &Mesh) -> Option<Mesh> {
    let pos = positions(mesh)?;
    let norm = normals(mesh)?;
    let tris = triangles(mesh)?;
    if pos.len() != norm.len() {
        return None;
    }
    let welded = weld(pos);
    let out = smooth_normals(pos, norm, &welded);

    // Thickness from the object's own extent.
    let mut lo = Vec3::splat(f32::MAX);
    let mut hi = Vec3::splat(f32::MIN);
    for p in pos {
        lo = lo.min(Vec3::from_array(*p));
        hi = hi.max(Vec3::from_array(*p));
    }
    let t = ((hi - lo).length() * SILHOUETTE).clamp(SILHOUETTE_MIN, SILHOUETTE_MAX);

    let pushed: Vec<[f32; 3]> = pos
        .iter()
        .zip(&out)
        .map(|(p, n)| (Vec3::from_array(*p) + *n * t).to_array())
        .collect();
    let indices: Vec<u32> = tris.iter().flat_map(|f| f.map(|i| i as u32)).collect();

    let mut m = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, pushed);
    m.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        out.iter().map(|n| n.to_array()).collect::<Vec<_>>(),
    );
    m.insert_indices(Indices::U32(indices));
    Some(m)
}

/// The folds inside the shape, as line primitives.
///
/// An edge belonging to one face only is a boundary — the open top of the cockpit
/// lining, the cut edge of a strake — and gets a line unconditionally, because an
/// opening is something a drawing shows.
pub fn creases(mesh: &Mesh) -> Option<(Mesh, usize)> {
    let pos = positions(mesh)?;
    let norm = normals(mesh)?;
    let tris = triangles(mesh)?;
    let welded = weld(pos);
    let out = smooth_normals(pos, norm, &welded);

    // Each welded edge, and the face normals that meet along it.
    let mut edges: HashMap<(usize, usize), EdgeFaces> = HashMap::new();
    for f in &tris {
        let p: Vec<Vec3> = f.iter().map(|&i| Vec3::from_array(pos[i])).collect();
        let Some(face) = (p[1] - p[0]).cross(p[2] - p[0]).try_normalize() else {
            continue; // A degenerate triangle has no opinion about which way is out.
        };
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            let (wa, wb) = (welded[a], welded[b]);
            let key = (wa.min(wb), wa.max(wb));
            edges
                .entry(key)
                .and_modify(|e| {
                    if e.1.is_some() {
                        e.2 = true; // A third face: these are touching sheets, not a fold.
                    } else {
                        e.1 = Some(face);
                    }
                })
                .or_insert((face, None, false, [a, b]));
        }
    }

    let cos_limit = CREASE_ANGLE.to_radians().cos();
    // Which edges fold at all.
    //
    // An edge carrying more than two faces is thrown out rather than tested. It is not
    // a fold; it is two separate sheets of surface that happen to touch along a line —
    // the cockpit lining against the inside of the planking, the deck against the
    // skirt tucked under it — and whichever two of its faces the test happens to
    // compare, the answer is meaningless.
    let mut folds: Vec<[usize; 2]> = Vec::new();
    let mut considered = 0usize;
    for (first, second, extra, ends) in edges.into_values() {
        if extra {
            continue;
        }
        considered += 1;
        let fold = match second {
            Some(other) => first.dot(other) < cos_limit,
            None => true,
        };
        if fold {
            folds.push(ends);
        }
    }

    // The share rule is a defence against *measured* geometry, so it is only applied to
    // geometry big enough to have been measured. A photogrammetric hull is twenty-seven
    // thousand triangles of surface with grain in it; a winch drum is forty triangles
    // somebody wrote down, and every fold in it is there on purpose. Testing the second
    // kind by the same rule threw away whole fittings the moment the crease angle was
    // lowered enough to find their chamfers — which is the opposite of what lowering it
    // was for.
    const MEASURED: usize = 2_000;
    if tris.len() > MEASURED && folds.len() as f32 > considered as f32 * GRAIN_SHARE {
        return None;
    }

    let mut line_pos: Vec<[f32; 3]> = Vec::new();
    let mut line_norm: Vec<[f32; 3]> = Vec::new();
    for ends in &folds {
        for &i in ends {
            let p = Vec3::from_array(pos[i]) + out[i] * CREASE_LIFT;
            line_pos.push(p.to_array());
            line_norm.push(out[i].to_array());
        }
    }
    let count = line_pos.len() / 2;
    if count == 0 {
        return None;
    }
    let mut m = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD);
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, line_pos);
    m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, line_norm);
    Some((m, count))
}

/// The colour the lines are drawn in: black.
///
/// It was a very dark indigo, on the honest argument that nothing in a lit world is
/// truly black. That argument is right about *shadows* and wrong about ink, which is
/// what this is: a drawn line is not a dark part of the scene, it is a mark made on top
/// of the scene, and a mark made in ink is made in ink. The tint only showed up as a
/// faint blue cast along every edge on the boat.
///
/// Not black. Black is what you reach for first and it is the one choice that cannot be
/// right, because nothing in a lit world is black — an outline in pure black reads as a
/// hole cut in the picture, and against water it reads as a hole with a boat in it.
/// Every ink tradition worth copying tints its darks: a comic inker's black is warm, a
/// cel painter's shadow line is the shadow colour of the scene. Here the scene is a
/// cold grey-blue sea under a northern sky, so the darks go indigo.
const INK: Vec3 = Vec3::ZERO;

/// How much of the object's own colour survives in its outline. None of it.
///
/// This was 0.3, on the reasoning that an outline carrying a little of its object's hue
/// keeps the varnished trim's line warm and the sail's line pale, so each line reads as
/// belonging to the thing it is drawn around. On the water that reasoning is sound. On
/// the boat it is wrong, and the picture settled it in one look: almost everything on
/// deck is wood, so almost every line came out brown, and a brown line round a brown
/// spar is not an outline — it is a smudge. The line has to be darker than anything it
/// borders or it stops separating them, which is the one job it has.
const INK_KEEPS: f32 = 0.0;

/// The outline colour for an object of a given colour.
fn ink_for(base: Color) -> Color {
    let c = base.to_linear();
    let own = Vec3::new(c.red, c.green, c.blue) * 0.42;
    let mix = INK.lerp(own, INK_KEEPS);
    Color::linear_rgb(mix.x, mix.y, mix.z)
}

/// Unlit and not casting a shadow.
///
/// Unlit is the whole point: a lit dark surface picks up the sky and the sun and stops
/// being a line, becoming a grey rim that looks like a modelling mistake. And the shell
/// must not cast a shadow or the boat sits in a halo of its own outline.
fn ink_material(
    materials: &mut Assets<StandardMaterial>,
    cull_front: bool,
    colour: Color,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: colour,
        unlit: true,
        cull_mode: if cull_front {
            Some(bevy::render::render_resource::Face::Front)
        } else {
            None
        },
        ..default()
    })
}

/// Build what is missing while the ink is on, and take it all off when it goes.
///
/// It runs every frame rather than on the keypress because the hull arrives late: it is
/// a scene load, so its meshes appear in the hierarchy some frames after the boat does,
/// and a one-shot build at toggle time would ink everything except the boat. Marking
/// each source entity as it is done makes the repeat free.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    mut commands: Commands,
    ink: Res<Ink>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<Entity, With<Inked>>,
    children: Query<&Children>,
    sources: InkSources,
    drawn: Query<Entity, With<Outline>>,
    marked: Query<Entity, With<Outlined>>,
) {
    if !ink.0 {
        if !drawn.is_empty() {
            for e in &drawn {
                commands.entity(e).despawn();
            }
            for e in &marked {
                commands.entity(e).remove::<Outlined>();
            }
        }
        return;
    }

    let mut lines = 0usize;
    let mut shells = 0usize;
    for root in &roots {
        for entity in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((entity, mesh3d, source_mat)) = sources.get(entity) else {
                continue;
            };
            let base = source_mat
                .and_then(|m| materials.get(&m.0))
                .map(|m| m.base_color)
                .unwrap_or(Color::srgb(0.5, 0.5, 0.5));
            let pen = ink_for(base);
            let built = {
                let Some(source) = meshes.get(&mesh3d.0) else {
                    continue; // Still loading. It will be here next frame.
                };
                (shell(source), creases(source))
            };
            commands.entity(entity).insert(Outlined);
            if let Some(hull) = built.0 {
                let handle = meshes.add(hull);
                let mat = ink_material(&mut materials, true, pen);
                commands.entity(entity).with_child((
                    Outline,
                    Mesh3d(handle),
                    MeshMaterial3d(mat),
                    bevy::light::NotShadowCaster,
                    bevy::light::NotShadowReceiver,
                ));
                shells += 1;
            }
            if let Some((fold, count)) = built.1 {
                let handle = meshes.add(fold);
                let mat = ink_material(&mut materials, false, pen);
                commands.entity(entity).with_child((
                    Outline,
                    Mesh3d(handle),
                    MeshMaterial3d(mat),
                    bevy::light::NotShadowCaster,
                    bevy::light::NotShadowReceiver,
                ));
                lines += count;
            }
        }
    }
    if shells > 0 {
        // Worth one line: a crease count in the tens of thousands means the threshold
        // has found a scan's noise rather than a boat's chines, and that is the number
        // to look at before anything else.
        info!("ink: {shells} silhouettes, {lines} crease lines");
    }
}
