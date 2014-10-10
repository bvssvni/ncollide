//! Ray casting utilities.

// types an traits
#[doc(inline)]
pub use ray::ray::{Ray, RayCast, RayIntersection};

// // functions
pub use ray::ray_plane::plane_toi_with_ray;
pub use ray::ray_implicit::implicit_toi_and_normal_with_ray;
pub use ray::ray_ball::ball_toi_with_ray;
pub use ray::ray_triangle::triangle_ray_intersection;

// modules
#[doc(hidden)]
pub mod ray;
mod ray_plane;
mod ray_ball;
mod ray_cuboid;
mod ray_aabb;
mod ray_bounding_sphere;
mod ray_implicit;
mod ray_triangle;
mod ray_concave;
mod ray_mesh;
mod ray_bvt;
mod ray_bezier_surface;
mod ray_bezier_curve;
