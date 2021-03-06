use utils::symbolic::{UnivariateFn, SymAdd, SymMult, SymSub, SymNeg, SymComp};
use utils::symbolic;
use math::Scalar;

/// The exponential function.
#[deriving(Clone)]
pub struct Exp;

/// The exponential function.
#[inline]
pub fn exp<A>(a: A) -> SymComp<Exp, A> {
    symbolic::comp(Exp, a)
}

impl<N: Scalar> UnivariateFn<N, N> for Exp {
    #[inline]
    fn d0(&self, t: N) -> N {
        t.exp()
    }

    #[inline]
    fn d1(&self, t: N) -> N {
        t.exp()
    }

    #[inline]
    fn d2(&self, t: N) -> N {
        t.exp()
    }

    #[inline]
    fn dn(&self, t: N, _: uint) -> N {
        t.exp()
    }
}

impl_ops_noparam!(Exp)
