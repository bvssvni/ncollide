use na::{AbsoluteRotate, Translate};
use na;
use bounding_volume::{HasAABB, AABB};
use shape::Cuboid;
use math::{Scalar, Point};

impl<N, P, V, M> HasAABB<P, M> for Cuboid<V>
    where N: Scalar,
          P: Point<N, V>,
          V: Neg<V>,
          M: Translate<P> + AbsoluteRotate<V> {
    #[inline]
    fn aabb(&self, m: &M) -> AABB<P> {
        let center          = m.translate(&na::orig());
        let ws_half_extents = m.absolute_rotate(self.half_extents());

        AABB::new(center + (-ws_half_extents), center + ws_half_extents)
    }
}
