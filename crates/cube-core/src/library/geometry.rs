//! Just enough solid geometry to cut a polyhedron into stickers.
//!
//! Every twisty puzzle here but Square-1 and Clock is a convex solid sliced by
//! planes, so one routine builds them all: each face is the polygon left when
//! the other faces clip a large square, and each cut splits those polygons.

/// A point or direction in space.
pub type Vec3 = [f64; 3];

/// How close two coordinates must be to count as the same.
pub const EPSILON: f64 = 1e-6;

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn scale(a: Vec3, factor: f64) -> Vec3 {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn subtract(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn normalized(a: Vec3) -> Vec3 {
    scale(a, 1. / dot(a, a).sqrt())
}

/// Whether two points are the same one.
pub fn same(a: Vec3, b: Vec3) -> bool {
    let gap = subtract(a, b);
    dot(gap, gap) < EPSILON
}

/// Turns `point` by `angle` radians around the unit vector `axis`, counter
/// clockwise as seen from the tip of the axis.
pub fn rotated(point: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (sine, cosine) = angle.sin_cos();
    add(
        add(scale(point, cosine), scale(cross(axis, point), sine)),
        scale(axis, dot(axis, point) * (1. - cosine)),
    )
}

/// A plane of the points `p` with `dot(normal, p) == offset`.
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub normal: Vec3,
    pub offset: f64,
}

/// Splits a convex polygon into what lies above and below `plane`. Either may
/// come back empty.
fn split(polygon: &[Vec3], plane: Plane) -> (Vec<Vec3>, Vec<Vec3>) {
    let (mut above, mut below) = (Vec::new(), Vec::new());
    for (index, &point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        let height = dot(plane.normal, point) - plane.offset;
        let next_height = dot(plane.normal, next) - plane.offset;
        if height >= -EPSILON {
            above.push(point);
        }
        if height <= EPSILON {
            below.push(point);
        }
        if (height > EPSILON && next_height < -EPSILON)
            || (height < -EPSILON && next_height > EPSILON)
        {
            let along = height / (height - next_height);
            let crossing = add(point, scale(subtract(next, point), along));
            above.push(crossing);
            below.push(crossing);
        }
    }
    (above, below)
}

/// The area of a planar polygon.
fn area(polygon: &[Vec3]) -> f64 {
    let mut total = [0.; 3];
    for (index, &point) in polygon.iter().enumerate() {
        total = add(total, cross(point, polygon[(index + 1) % polygon.len()]));
    }
    dot(total, total).sqrt() / 2.
}

/// The middle of a polygon's corners.
pub fn centroid(polygon: &[Vec3]) -> Vec3 {
    let total = polygon.iter().fold([0.; 3], |sum, &point| add(sum, point));
    scale(total, 1. / polygon.len() as f64)
}

/// Returns the polygon of the face on `faces[index]` of the convex solid.
pub fn face_polygon(faces: &[Plane], index: usize) -> Vec<Vec3> {
    let face = faces[index];
    let helper = if face.normal[0].abs() < 0.9 {
        [1., 0., 0.]
    } else {
        [0., 1., 0.]
    };
    let across = normalized(cross(face.normal, helper));
    let up = cross(face.normal, across);
    let middle = scale(face.normal, face.offset);
    let reach = 100.;
    let corner = |a: f64, b: f64| add(middle, add(scale(across, a), scale(up, b)));
    let mut polygon = vec![
        corner(-reach, -reach),
        corner(reach, -reach),
        corner(reach, reach),
        corner(-reach, reach),
    ];
    for (other, plane) in faces.iter().enumerate() {
        if other != index {
            polygon = split(&polygon, *plane).1;
        }
    }
    polygon
}

/// Cuts `polygon` along every plane in `cuts`, leaving out slivers.
pub fn cut(polygon: Vec<Vec3>, cuts: &[Plane]) -> Vec<Vec<Vec3>> {
    let mut pieces = vec![polygon];
    for &plane in cuts {
        let mut next = Vec::with_capacity(pieces.len() * 2);
        for piece in pieces {
            let (above, below) = split(&piece, plane);
            next.extend([above, below]);
        }
        next.retain(|piece| piece.len() >= 3 && area(piece) > EPSILON);
        pieces = next;
    }
    pieces
}
