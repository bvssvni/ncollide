// XXX: implement this for 2d too.

use na::{FloatVec, Outer, EigenQR, Pnt3, Mat3, Zero};
use na;
use utils;
use procedural::{TriMesh, IndexBuffer};
use procedural;
use volumetric::Volumetric;
use math::{Scalar, Point, Vect};
use shape::Convex3;


fn tetrahedron_unit_inertia_tensor_wrt_point<N, P, V, I>(point: &P, p1: &P, p2: &P, p3: &P, p4: &P) -> I
    where N: Scalar,
          P: Point<N, V>,
          V: FloatVec<N>,
          I: Zero + IndexMut<(uint, uint), N> {
    assert!(na::dim::<P>() == 3);

    let p1 = *p1 - *point;
    let p2 = *p2 - *point;
    let p3 = *p3 - *point;
    let p4 = *p4 - *point;

    let _frac_10: N = na::cast(0.1f64);
    let _frac_20: N = na::cast(0.05f64);
    let _2      : N = na::cast(2.0f64);

    // Just for readability.
    let x1 = p1[0]; let y1 = p1[1]; let z1 = p1[2];
    let x2 = p2[0]; let y2 = p2[1]; let z2 = p2[2];
    let x3 = p3[0]; let y3 = p3[1]; let z3 = p3[2];
    let x4 = p4[0]; let y4 = p4[1]; let z4 = p4[2];

    let diag_x = x1 * x1 + x1 * x2 + x2 * x2 + x1 * x3 + x2 * x3 + x3 * x3 + x1 * x4 + x2 * x4 + x3 * x4 + x4 * x4;
    let diag_y = y1 * y1 + y1 * y2 + y2 * y2 + y1 * y3 + y2 * y3 + y3 * y3 + y1 * y4 + y2 * y4 + y3 * y4 + y4 * y4;
    let diag_z = z1 * z1 + z1 * z2 + z2 * z2 + z1 * z3 + z2 * z3 + z3 * z3 + z1 * z4 + z2 * z4 + z3 * z4 + z4 * z4;

    let a0 = (diag_y + diag_z) * _frac_10;
    let b0 = (diag_z + diag_x) * _frac_10;
    let c0 = (diag_x + diag_y) * _frac_10;

    let a1 = (y1 * z1 * _2 + y2 * z1      + y3 * z1      + y4 * z1 +
              y1 * z2      + y2 * z2 * _2 + y3 * z2      + y4 * z2 +
              y1 * z3      + y2 * z3      + y3 * z3 * _2 + y4 * z3 +
              y1 * z4      + y2 * z4      + y3 * z4      + y4 * z4 * _2) * _frac_20;
    let b1 = (x1 * z1 * _2 + x2 * z1      + x3 * z1      + x4 * z1 +
              x1 * z2      + x2 * z2 * _2 + x3 * z2      + x4 * z2 +
              x1 * z3      + x2 * z3      + x3 * z3 * _2 + x4 * z3 +
              x1 * z4      + x2 * z4      + x3 * z4      + x4 * z4 * _2) * _frac_20;
    let c1 = (x1 * y1 * _2 + x2 * y1      + x3 * y1      + x4 * y1 +
              x1 * y2      + x2 * y2 * _2 + x3 * y2      + x4 * y2 +
              x1 * y3      + x2 * y3      + x3 * y3 * _2 + x4 * y3 +
              x1 * y4      + x2 * y4      + x3 * y4      + x4 * y4 * _2) * _frac_20;

    let mut res = na::zero::<I>();

    res[(0, 0)] =  a0; res[(0, 1)] = -b1; res[(0, 2)] = -c1;
    res[(1, 0)] = -b1; res[(1, 1)] =  b0; res[(1, 2)] = -a1;
    res[(2, 0)] = -c1; res[(2, 1)] = -a1; res[(2, 2)] =  c0;

    res
}

/// The volume and center of mass of a convex mesh.
///
/// This is unsafe as the mesh is not checked to be actually convex.
pub unsafe fn convex_mesh_volume_and_center_of_mass<N, P, V>(convex_mesh: &TriMesh<N, P, V>) -> (N, P)
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> {
    let geometric_center = utils::center(convex_mesh.coords.as_slice());

    let mut res = na::orig::<P>();
    let mut vol = na::zero::<N>();

    match convex_mesh.indices {
        IndexBuffer::Unified(ref idx) => {
            for t in idx.iter() {
                let p2 = &convex_mesh.coords[t.x as uint];
                let p3 = &convex_mesh.coords[t.y as uint];
                let p4 = &convex_mesh.coords[t.z as uint];

                let volume = utils::tetrahedron_volume(&geometric_center, p2, p3, p4);
                let center = utils::tetrahedron_center(&geometric_center, p2, p3, p4);

                res = res + *center.as_vec() * volume;
                vol = vol + volume;
            }
        },
        IndexBuffer::Split(_) => unreachable!()
    }

    if na::is_zero(&vol) {
        (vol, geometric_center)
    }
    else {
        (vol, res / vol)
    }
}

/// The mass properties of a convex mesh.
///
/// This is unsafe as the mesh is not checked to be actually convex.
pub unsafe fn convex_mesh_mass_properties<N, P, V, I>(convex_mesh: &TriMesh<N, P, V>,
                                                      density:     N)
                                                      -> (N, P, I)
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N>,
          I: Zero + Add<I, I> + Mul<N, I> + IndexMut<(uint, uint), N> {
    assert!(na::dim::<P>() == 3);

    let (volume, com) = convex_mesh_volume_and_center_of_mass(convex_mesh);

    if na::is_zero(&volume) {
        return (na::zero(), com, na::zero());
    }

    let mut itot = na::zero::<I>();

    match convex_mesh.indices {
        IndexBuffer::Unified(ref idx) => {
            for t in idx.iter() {
                let p2 = &convex_mesh.coords[t.x as uint];
                let p3 = &convex_mesh.coords[t.y as uint];
                let p4 = &convex_mesh.coords[t.z as uint];

                let vol      = utils::tetrahedron_volume(&com, p2, p3, p4);
                let ipart: I = tetrahedron_unit_inertia_tensor_wrt_point(&com, &com, p2, p3, p4);

                itot = itot + ipart * vol;
            }
        },
        IndexBuffer::Split(_) => unreachable!()
    }

    (volume * density, com, itot * density)
}

/// The surface of a convex mesh.
///
/// This is unsafe as the mesh is not checked to be actually convex.
pub unsafe fn convex_mesh_surface<N, P, V>(convex_mesh: &TriMesh<N, P, V>) -> N
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> {
    let mut surface = na::zero::<N>();

    match convex_mesh.indices {
        IndexBuffer::Unified(ref idx) => {
            for t in idx.iter() {
                let p1 = &convex_mesh.coords[t.x as uint];
                let p2 = &convex_mesh.coords[t.y as uint];
                let p3 = &convex_mesh.coords[t.z as uint];

                surface = surface + utils::triangle_area(p1, p2, p3);
            }
        },
        IndexBuffer::Split(_) => unreachable!()
    }

    surface
}

/// The surface of a convex hull.
pub fn convex_hull_surface<N, P, V, M>(dim: uint, points: &[P]) -> N
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> + Outer<M>,
          M: EigenQR<N, V> + Mul<P, P> + Add<M, M> + Zero {
    assert!(dim == 2 || dim == 3);

    match dim {
        2 => {
            unimplemented!()
        }
        3 => {
            let convex_mesh = procedural::convex_hull3(points);
            unsafe { convex_mesh_surface(&convex_mesh) }
        }
        _ => {
            unimplemented!()
        }
    }
}

/// The volume of the convex hull of a set of points.
pub fn convex_hull_volume<N, P, V, M>(dim: uint, points: &[P]) -> N
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> + Outer<M>,
          M: EigenQR<N, V> + Mul<P, P> + Add<M, M> + Zero {
    assert!(dim == 2 || dim == 3);

    match dim {
        2 => {
            unimplemented!()
        }
        3 => {
            let convex_mesh = procedural::convex_hull3(points);
            unsafe { convex_mesh_volume_and_center_of_mass(&convex_mesh).val0() }
        }
        _ => {
            unimplemented!()
        }
    }
}

/// The center of mass of the convex hull of a set of points.
pub fn convex_hull_center_of_mass<N, P, V, M>(dim: uint, points: &[P]) -> P
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> + Outer<M>,
          M: EigenQR<N, V> + Mul<P, P> + Add<M, M> + Zero {
    assert!(dim == 2 || dim == 3);

    match dim {
        2 => {
            unimplemented!()
        }
        3 => {
            let convex_mesh = procedural::convex_hull3(points);
            unsafe { convex_mesh_volume_and_center_of_mass(&convex_mesh).val1() }
        }
        _ => {
            unimplemented!()
        }
    }
}

/// The angular inertia of the convex hull of a set of points.
pub fn convex_hull_unit_angular_inertia<N, P, V, M, I>(dim: uint, points: &[P]) -> I
    where N: Scalar,
          P: Point<N, V>,
          V: Vect<N> + Outer<M>,
          M: EigenQR<N, V> + Mul<P, P> + Add<M, M> + Zero,
          I: Zero + Add<I, I> + Mul<N, I> + IndexMut<(uint, uint), N> {
    assert!(dim == 2 || dim == 3);

    match dim {
        2 => {
            unimplemented!()
        }
        3 => {
            let convex_mesh = procedural::convex_hull3(points);
            unsafe {
                let (vol, _, i): (N, _, I) = convex_mesh_mass_properties(&convex_mesh, na::one());

                i * (na::one::<N>() / vol)
            }
        }
        _ => {
            unimplemented!()
        }
    }
}

impl<N: Scalar> Volumetric<N, Pnt3<N>, Mat3<N>> for Convex3<N> {
    fn surface(&self) -> N {
        convex_hull_surface(3, self.points())
    }

    fn volume(&self) -> N {
        convex_hull_volume(3, self.points())
    }

    fn center_of_mass(&self) -> Pnt3<N> {
        convex_hull_center_of_mass(3, self.points())
    }

    fn unit_angular_inertia(&self) -> Mat3<N> {
        convex_hull_unit_angular_inertia(3, self.points())
    }

    fn mass_properties(&self, density: N) -> (N, Pnt3<N>, Mat3<N>) {
        let convex_mesh = procedural::convex_hull3(self.points());
        unsafe { convex_mesh_mass_properties(&convex_mesh, density) }
    }
}

#[cfg(test)]
mod test {
    use na::Vec3;
    use na;
    use shape::{Convex, Cuboid};
    use procedural;
    use volumetric::Volumetric;

    #[test]
    fn test_inertia_tensor() {
        let excentricity = 10.0;

        let mut shape = procedural::cuboid(&Vec3::new(2.0f64 - 0.08, 2.0 - 0.08, 2.0 - 0.08));

        for c in shape.coords.iter_mut() {
            c.x = c.x + excentricity;
            c.y = c.y + excentricity;
            c.z = c.z + excentricity;
        }

        let convex = Convex::new(shape.coords);
        let cuboid = Cuboid::new(Vec3::new(0.96f64, 0.96, 0.96));

        let actual   = convex.unit_angular_inertia();
        let expected = cuboid.unit_angular_inertia();

        assert!(na::approx_eq(&actual, &expected),
                format!("Inertia tensors do not match: actual {}, expected: {}.", actual, expected));

        let (actual_m, _, actual_i) = convex.mass_properties(2.37689);
        let (expected_m, _, expected_i) = cuboid.mass_properties(2.37689);

        assert!(na::approx_eq(&actual, &expected),
                format!("Unit inertia tensors do not match: actual {}, expected: {}.", actual, expected));

        assert!(na::approx_eq(&actual_i, &expected_i),
                format!("Inertia tensors do not match: actual {}, expected: {}.", actual_i, expected_i));

        assert!(na::approx_eq(&actual_m, &expected_m),
                format!("Masses do not match: actual {}, expected: {}.", actual_m, expected_m));
    }
}
