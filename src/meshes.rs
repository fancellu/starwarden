//! Procedural art: every shape in the game is built here from coloured triangles. No image
//! files are used; the only texture is a generated soft glow dot.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::AlphaMode2d;
use std::f32::consts::{PI, TAU};

use crate::consts::*;

type P = (f32, f32);

/// Accumulates vertex-coloured triangles.
#[derive(Default)]
pub struct Shape {
    pos: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

fn arr(c: LinearRgba) -> [f32; 4] {
    [c.red, c.green, c.blue, c.alpha]
}

impl Shape {
    fn v(&mut self, p: P, c: LinearRgba) -> u32 {
        self.pos.push([p.0, p.1, 0.0]);
        self.col.push(arr(c));
        (self.pos.len() - 1) as u32
    }

    /// Triangle fan around `center`; works for any polygon that is star-shaped from it.
    pub fn fan(&mut self, center: P, pts: &[P], cc: LinearRgba, ce: LinearRgba) -> &mut Self {
        let c = self.v(center, cc);
        let first = self.pos.len() as u32;
        for &p in pts {
            self.v(p, ce);
        }
        let n = pts.len() as u32;
        for i in 0..n {
            self.idx.extend([c, first + i, first + (i + 1) % n]);
        }
        self
    }

    /// Flat-coloured polygon (fan around its centroid).
    pub fn poly(&mut self, pts: &[P], c: LinearRgba) -> &mut Self {
        let n = pts.len() as f32;
        let cx = pts.iter().map(|p| p.0).sum::<f32>() / n;
        let cy = pts.iter().map(|p| p.1).sum::<f32>() / n;
        self.fan((cx, cy), pts, c, c)
    }

    pub fn quad(&mut self, pts: [P; 4], cols: [LinearRgba; 4]) -> &mut Self {
        let a = self.v(pts[0], cols[0]);
        let b = self.v(pts[1], cols[1]);
        let c = self.v(pts[2], cols[2]);
        let d = self.v(pts[3], cols[3]);
        self.idx.extend([a, b, c, a, c, d]);
        self
    }

    pub fn circle(
        &mut self,
        center: P,
        r: f32,
        n: usize,
        cc: LinearRgba,
        ce: LinearRgba,
    ) -> &mut Self {
        let pts: Vec<P> = (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * TAU;
                (center.0 + a.cos() * r, center.1 + a.sin() * r)
            })
            .collect();
        self.fan(center, &pts, cc, ce)
    }

    /// Regular polygon ring between two radii.
    pub fn ring(
        &mut self,
        center: P,
        r0: f32,
        r1: f32,
        n: usize,
        rot: f32,
        c0: LinearRgba,
        c1: LinearRgba,
    ) -> &mut Self {
        for i in 0..n {
            let a0 = rot + i as f32 / n as f32 * TAU;
            let a1 = rot + (i + 1) as f32 / n as f32 * TAU;
            let p = |a: f32, r: f32| (center.0 + a.cos() * r, center.1 + a.sin() * r);
            self.quad(
                [p(a0, r0), p(a0, r1), p(a1, r1), p(a1, r0)],
                [c0, c1, c1, c0],
            );
        }
        self
    }

    pub fn build(&mut self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, std::mem::take(&mut self.pos))
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut self.col))
        .with_inserted_indices(Indices::U32(std::mem::take(&mut self.idx)))
    }
}

/// Accumulates textured, tinted quads that all share the glow material, so that any number
/// of stars, particles and shots are drawn with one mesh and one draw call.
///
/// Bevy gives every sprite with its own colour or size a separate material, which is far too
/// slow for hundreds of fading particles; this is the cheap alternative.
#[derive(Default)]
pub struct GlowQuads {
    pos: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl GlowQuads {
    /// Adds a quad. `soft` uses the round glow; otherwise the quad is a solid rectangle.
    pub fn push(&mut self, center: Vec2, size: Vec2, color: LinearRgba, soft: bool) {
        let base = self.pos.len() as u32;
        let h = size * 0.5;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            self.pos
                .push([center.x + sx * h.x, center.y + sy * h.y, 0.0]);
            // The centre of the glow texture is fully opaque, so pointing every corner at it
            // gives a solid colour.
            self.uv.push(if soft {
                [0.5 + sx * 0.5, 0.5 - sy * 0.5]
            } else {
                [0.5, 0.5]
            });
            self.col.push(arr(color));
        }
        self.idx
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    /// Moves the quads into `mesh`, replacing what it held.
    pub fn write_to(&mut self, mesh: &mut Mesh) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, std::mem::take(&mut self.pos));
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, std::mem::take(&mut self.uv));
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut self.col));
        mesh.insert_indices(Indices::U32(std::mem::take(&mut self.idx)));
    }

    pub fn build(&mut self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        self.write_to(&mut mesh);
        mesh
    }
}

/// Handles to all shared meshes, the single vertex-colour material and the glow texture.
#[derive(Resource)]
pub struct Art {
    pub mat: Handle<ColorMaterial>,
    pub glow: Handle<Image>,
    /// Material for [`GlowQuads`] meshes: the glow texture tinted by vertex colour.
    pub glow_mat: Handle<ColorMaterial>,
    pub ship: Handle<Mesh>,
    pub flame: Handle<Mesh>,
    pub missile: Handle<Mesh>,
    pub colonist: Handle<Mesh>,
    /// Indexed `[normal, elite]`.
    pub harvester: [Handle<Mesh>; 2],
    pub wraith: [Handle<Mesh>; 2],
    pub hive: [Handle<Mesh>; 2],
    pub stinger: [Handle<Mesh>; 2],
    pub stalker: [Handle<Mesh>; 2],
    /// A falling shield pickup.
    pub shield: Handle<Mesh>,
}

fn dim(c: LinearRgba, f: f32) -> LinearRgba {
    LinearRgba {
        red: c.red * f,
        green: c.green * f,
        blue: c.blue * f,
        alpha: c.alpha,
    }
}

fn ship() -> Mesh {
    let mut s = Shape::default();
    // Swept wings.
    s.poly(&[(2.0, 4.0), (-17.0, 15.0), (-12.0, 4.0)], C_MAGENTA);
    s.poly(&[(2.0, -4.0), (-17.0, -15.0), (-12.0, -4.0)], C_MAGENTA);
    // Hull.
    s.fan(
        (-2.0, 0.0),
        &[
            (24.0, 0.0),
            (6.0, 5.5),
            (-10.0, 6.5),
            (-17.0, 4.0),
            (-13.0, 0.0),
            (-17.0, -4.0),
            (-10.0, -6.5),
            (6.0, -5.5),
        ],
        C_WHITE,
        C_CYAN,
    );
    // Canopy.
    s.poly(
        &[(12.0, 0.0), (3.0, 3.0), (-3.0, 0.0), (3.0, -3.0)],
        rgb(0.02, 0.1, 0.3),
    );
    s.build()
}

fn flame() -> Mesh {
    let mut s = Shape::default();
    s.fan(
        (-4.0, 0.0),
        &[(0.0, 3.2), (-26.0, 0.0), (0.0, -3.2)],
        C_WHITE,
        with_alpha(C_FLAME, 0.0),
    );
    s.poly(&[(0.0, 3.2), (-9.0, 0.0), (0.0, -3.2)], C_FLAME);
    s.build()
}

fn missile() -> Mesh {
    let mut s = Shape::default();
    s.poly(
        &[
            (9.0, 0.0),
            (3.0, 2.5),
            (-7.0, 2.5),
            (-7.0, -2.5),
            (3.0, -2.5),
        ],
        C_WHITE,
    );
    s.poly(&[(-4.0, 2.5), (-9.0, 6.0), (-7.0, 2.5)], C_AMBER);
    s.poly(&[(-4.0, -2.5), (-9.0, -6.0), (-7.0, -2.5)], C_AMBER);
    s.build()
}

fn colonist() -> Mesh {
    let mut s = Shape::default();
    // Helmet with visor.
    s.circle((0.0, 7.5), 3.4, 10, C_WHITE, C_PEACH);
    s.poly(
        &[(-2.2, 8.2), (2.2, 8.2), (2.2, 6.6), (-2.2, 6.6)],
        C_CYAN_DIM,
    );
    // Suit and legs.
    s.poly(
        &[(-3.2, 4.0), (3.2, 4.0), (2.4, -3.0), (-2.4, -3.0)],
        C_PEACH,
    );
    s.poly(
        &[(-2.4, -3.0), (-0.4, -3.0), (-1.0, -8.0), (-3.0, -8.0)],
        dim(C_PEACH, 0.6),
    );
    s.poly(
        &[(0.4, -3.0), (2.4, -3.0), (3.0, -8.0), (1.0, -8.0)],
        dim(C_PEACH, 0.6),
    );
    // Backpack beacon.
    s.circle((0.0, 1.2), 1.2, 6, C_WHITE, C_GREEN);
    s.build()
}

/// A clawed collector craft: domed top, wide skirt and two grasping arms.
fn harvester(elite: bool) -> Mesh {
    let (a, b) = if elite {
        (C_RED, C_RED_DIM)
    } else {
        (C_GREEN, C_GREEN_DIM)
    };
    let mut s = Shape::default();
    // Dome.
    let dome: Vec<P> = (0..=8)
        .map(|i| {
            let ang = i as f32 / 8.0 * PI;
            (ang.cos() * 9.0, 3.0 + ang.sin() * 9.0)
        })
        .collect();
    s.fan((0.0, 5.0), &dome, C_WHITE, a);
    // Skirt.
    s.quad(
        [(-16.0, 3.0), (16.0, 3.0), (11.0, -4.0), (-11.0, -4.0)],
        [a, a, b, b],
    );
    // Claws.
    s.poly(
        &[(-11.0, -4.0), (-7.0, -4.0), (-12.0, -13.0), (-15.0, -9.0)],
        a,
    );
    s.poly(&[(11.0, -4.0), (7.0, -4.0), (12.0, -13.0), (15.0, -9.0)], a);
    s.poly(&[(-12.0, -13.0), (-15.0, -9.0), (-8.0, -15.0)], C_WHITE);
    s.poly(&[(12.0, -13.0), (15.0, -9.0), (8.0, -15.0)], C_WHITE);
    // Running lights.
    for x in [-10.0, 0.0, 10.0] {
        s.circle((x, -0.5), 1.6, 6, C_WHITE, C_AMBER);
    }
    s.build()
}

/// A jagged, pulsing star.
fn wraith(elite: bool) -> Mesh {
    let (a, b) = if elite {
        (C_RED, C_AMBER)
    } else {
        (C_MAGENTA, C_VIOLET)
    };
    let mut s = Shape::default();
    let pts: Vec<P> = (0..16)
        .map(|i| {
            let ang = i as f32 / 16.0 * TAU;
            let r = if i % 2 == 0 {
                if i % 4 == 0 {
                    17.0
                } else {
                    12.0
                }
            } else {
                5.5
            };
            (ang.cos() * r, ang.sin() * r)
        })
        .collect();
    s.fan((0.0, 0.0), &pts, C_WHITE, a);
    s.ring((0.0, 0.0), 3.0, 5.0, 8, 0.0, b, b);
    s.circle((0.0, 0.0), 2.4, 8, rgb(0.0, 0.0, 0.0), rgb(0.1, 0.0, 0.1));
    s.build()
}

/// A hexagonal pod with a glowing core.
fn hive(elite: bool) -> Mesh {
    let (a, b) = if elite {
        (C_RED, C_RED_DIM)
    } else {
        (C_AMBER, C_AMBER_DIM)
    };
    let mut s = Shape::default();
    s.ring((0.0, 0.0), 13.0, 19.0, 6, 0.0, b, a);
    s.ring((0.0, 0.0), 7.0, 10.0, 6, PI / 6.0, a, b);
    s.circle((0.0, 0.0), 5.0, 6, C_WHITE, a);
    for i in 0..6 {
        let ang = i as f32 / 6.0 * TAU;
        s.circle((ang.cos() * 19.0, ang.sin() * 19.0), 2.4, 6, C_WHITE, a);
    }
    s.build()
}

/// A small dart.
fn stinger(elite: bool) -> Mesh {
    let a = if elite { C_RED } else { C_AMBER };
    let mut s = Shape::default();
    s.fan(
        (0.0, 0.0),
        &[(8.0, 0.0), (0.0, 4.5), (-8.0, 0.0), (0.0, -4.5)],
        C_WHITE,
        a,
    );
    s.poly(&[(-4.0, 3.0), (-11.0, 7.0), (-7.0, 0.5)], a);
    s.poly(&[(-4.0, -3.0), (-11.0, -7.0), (-7.0, -0.5)], a);
    s.build()
}

/// A thin, wide blade with a central eye.
fn stalker(elite: bool) -> Mesh {
    let (a, b) = if elite {
        (C_RED, C_AMBER)
    } else {
        (C_BLUE, C_CYAN)
    };
    let mut s = Shape::default();
    s.fan(
        (0.0, 0.0),
        &[
            (26.0, 0.0),
            (8.0, 4.5),
            (-8.0, 4.5),
            (-26.0, 0.0),
            (-8.0, -4.5),
            (8.0, -4.5),
        ],
        C_WHITE,
        a,
    );
    s.poly(&[(14.0, 3.0), (22.0, 9.0), (18.0, 1.5)], b);
    s.poly(&[(-14.0, 3.0), (-22.0, 9.0), (-18.0, 1.5)], b);
    s.poly(&[(14.0, -3.0), (22.0, -9.0), (18.0, -1.5)], b);
    s.poly(&[(-14.0, -3.0), (-22.0, -9.0), (-18.0, -1.5)], b);
    s.circle((0.0, 0.0), 2.6, 8, rgb(0.0, 0.0, 0.0), rgb(0.0, 0.05, 0.2));
    s.build()
}

/// A cyan hexagon with a bright core: a shield waiting to be caught.
fn shield() -> Mesh {
    let mut s = Shape::default();
    s.ring((0.0, 0.0), 7.5, 10.5, 6, PI / 6.0, C_CYAN_DIM, C_CYAN);
    s.circle((0.0, 0.0), 4.5, 6, C_WHITE, C_CYAN);
    s.build()
}

/// Soft radial falloff used for particles, stars, shots and nebula clouds.
fn glow_image() -> Image {
    const N: u32 = 64;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let d = Vec2::new(
                x as f32 + 0.5 - N as f32 / 2.0,
                y as f32 + 0.5 - N as f32 / 2.0,
            )
            .length()
                / (N as f32 / 2.0);
            let a = (1.0 - d).clamp(0.0, 1.0);
            let a = a * a * (3.0 - 2.0 * a);
            data.extend([255, 255, 255, (a * a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn setup_art(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut pair = |f: fn(bool) -> Mesh| [meshes.add(f(false)), meshes.add(f(true))];
    let harvester = pair(harvester);
    let wraith = pair(wraith);
    let hive = pair(hive);
    let stinger = pair(stinger);
    let stalker = pair(stalker);
    let glow = images.add(glow_image());
    commands.insert_resource(Art {
        mat: materials.add(ColorMaterial::default()),
        glow_mat: materials.add(ColorMaterial {
            texture: Some(glow.clone()),
            alpha_mode: AlphaMode2d::Blend,
            ..default()
        }),
        glow,
        ship: meshes.add(ship()),
        flame: meshes.add(flame()),
        missile: meshes.add(missile()),
        colonist: meshes.add(colonist()),
        harvester,
        wraith,
        hive,
        stinger,
        stalker,
        shield: meshes.add(shield()),
    });
}
