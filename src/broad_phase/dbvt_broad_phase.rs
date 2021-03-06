use std::cell::RefCell;
use std::rc::Rc;
use na::Translation;
use na;
use broad_phase::BroadPhase;
use utils::data::hash::UintTWHash;
use utils::data::hash_map::HashMap;
use utils::data::pair::{Pair, PairTWHash};
use utils::data::has_uid::HasUid;
use broad_phase::Dispatcher;
use bounding_volume::{HasBoundingVolume, BoundingVolume};
use ray::{Ray, LocalRayCast};
use point::LocalPointQuery;
use partitioning::{DBVT, DBVTLeaf,
                   BoundingVolumeInterferencesCollector,
                   RayInterferencesCollector,
                   PointInterferencesCollector};
use math::{Scalar, Point, Vect};


/// Broad phase based on a Dynamic Bounding Volume Tree.
///
/// It uses two separate trees: one for static objects and which is never updated, and one for
/// moving objects.
pub struct DBVTBroadPhase<N, P, B, BV, D, DV> {
    tree:        DBVT<P, B, BV>,
    stree:       DBVT<P, B, BV>,
    active2bv:   HashMap<uint, Rc<RefCell<DBVTLeaf<P, B, BV>>>, UintTWHash>,
    inactive2bv: HashMap<uint, Rc<RefCell<DBVTLeaf<P, B, BV>>>, UintTWHash>,
    pairs:       HashMap<Pair<Rc<RefCell<DBVTLeaf<P, B, BV>>>>, DV, PairTWHash>, // pair manager
    spairs:      HashMap<Pair<Rc<RefCell<DBVTLeaf<P, B, BV>>>>, DV, PairTWHash>,
    dispatcher:  D,
    margin:      N,
    collector:   Vec<Rc<RefCell<DBVTLeaf<P, B, BV>>>>,
    to_update:   Vec<Rc<RefCell<DBVTLeaf<P, B, BV>>>>,
    update_off:  uint // incremental pairs removal index
}

impl<N, P, V, B, BV, D, DV> DBVTBroadPhase<N, P, B, BV, D, DV>
    where N:  Scalar,
          P:  Point<N, V>,
          V:  Vect<N>,
          B:  'static + HasBoundingVolume<BV> + Clone,
          BV: 'static + BoundingVolume<N> + Translation<V> + Clone,
          D:  Dispatcher<B, B, DV> {
    /// Creates a new broad phase based on a Dynamic Bounding Volume Tree.
    pub fn new(dispatcher: D, margin: N) -> DBVTBroadPhase<N, P, B, BV, D, DV> {
        DBVTBroadPhase {
            tree:        DBVT::new(),
            stree:       DBVT::new(),
            active2bv:   HashMap::new(UintTWHash::new()),
            inactive2bv: HashMap::new(UintTWHash::new()),
            pairs:       HashMap::new(PairTWHash::new()),
            spairs:      HashMap::new(PairTWHash::new()),
            dispatcher:  dispatcher,
            update_off:  0,
            collector:   Vec::new(),
            to_update:   Vec::new(),
            margin:      margin
        }
    }

    /// Number of interferences detected by this broad phase.
    #[inline]
    pub fn num_interferences(&self) -> uint {
        self.pairs.len()
    }

    fn update_updatable(&mut self) {
        /*
         * Re-insert outdated nodes one by one and collect interferences at the same time.
         */
        let mut new_colls = 0u;

        for u in self.to_update.iter() {
            { // scope to avoid dynamic borrow failure.
                let bu = u.borrow();
                self.tree.interferences_with_leaf(bu.deref(), &mut self.collector);
                self.stree.interferences_with_leaf(bu.deref(), &mut self.collector);

                // dispatch
                for i in self.collector.iter() {
                    let bi = i.borrow();
                    if self.dispatcher.is_valid(&bu.object, &bi.object) {
                        let dispatcher = &mut self.dispatcher;
                        let _ = self.pairs.find_or_insert_lazy(
                            Pair::new(u.clone(), i.clone()),
                            || dispatcher.dispatch(&bu.object, &bi.object)
                            );

                        new_colls = new_colls + 1;
                    }
                }
            }

            self.collector.clear();
            self.tree.insert(u.clone());
        }

        /*
         * Remove some of the outdated collisions.
         */
        // NOTE: the exact same code is used on `brute_force_bounding_volume_broad_phase.rs`.
        // Refactor that?
        if new_colls != 0 && self.pairs.len() != 0 {
            let len          = self.pairs.len();
            let num_removals = na::clamp(new_colls, len / 10, len);

            for i in range(self.update_off, self.update_off + num_removals) {
                let id = i % self.pairs.len();

                let remove = {
                    let elts  = self.pairs.elements();
                    let entry = &elts[id];

                    let bf = entry.key.first.borrow();
                    let bs = entry.key.second.borrow();
                    if !bf.bounding_volume.intersects(&bs.bounding_volume) {
                        true
                    }
                    else {
                        false
                    }
                };

                if remove {
                    self.pairs.remove_elem_at(id);
                }
            }

            self.update_off = (self.update_off + num_removals) % self.pairs.len();
        }

        self.to_update.clear();
    }
}

impl<N, P, V, B, BV, D, DV> BroadPhase<P, V, B, BV, DV> for DBVTBroadPhase<N, P, B, BV, D, DV>
    where N:  Scalar,
          P:  Point<N, V>,
          V:  Vect<N>,
          B:  'static + HasBoundingVolume<BV> + HasUid + Clone,
          BV: 'static + BoundingVolume<N> + Translation<V> + LocalRayCast<N, P, V> + LocalPointQuery<N, P> + Clone,
          D:  Dispatcher<B, B, DV> {
    #[inline]
    fn add(&mut self, b: B) {
        let id   = b.uid();
        let leaf = Rc::new(RefCell::new(DBVTLeaf::new(b.bounding_volume().loosened(self.margin.clone()), b)));

        self.to_update.push(leaf.clone());
        self.update_updatable();

        self.active2bv.insert(id, leaf);
    }

    fn remove(&mut self, b: &B) {
        // remove b from the dbvts
        let key      = b.uid();
        let leaf_opt = self.active2bv.get_and_remove(&key);
        let mut leaf;

        match leaf_opt {
            Some(l) => {
                leaf = l.value;
                self.tree.remove(&mut leaf);
            },
            None => {
                let leaf_opt = self.inactive2bv.get_and_remove(&key);
                match leaf_opt {
                    Some(l) => {
                        leaf = l.value;
                        self.stree.remove(&mut leaf);
                    },
                    None => return
                }
            }
        }

        let mut keys_to_remove = Vec::new();

        // remove every pair involving b
        for elt in self.pairs.elements().iter() {
            if elt.key.first.uid() == leaf.uid() || elt.key.second.uid() == leaf.uid() {
                keys_to_remove.push(elt.key.clone());
            }
        }

        for k in keys_to_remove.iter() {
            self.pairs.remove(k);
        }

        keys_to_remove.clear();

        // remove every "sleeping" pair involving b
        for elt in self.spairs.elements().iter() {
            if elt.key.first.uid() == leaf.uid() || elt.key.second.uid() == leaf.uid() {
                keys_to_remove.push(elt.key.clone());
            }
        }

        for k in keys_to_remove.iter() {
            self.spairs.remove(k);
        }
    }

    fn update(&mut self) {
        // NOTE: be careful not to add the same object twice!
        /*
         * Remove all outdated nodes
         */
        for a in self.active2bv.elements_mut().iter_mut() {
            let mut new_bv = a.value.borrow().object.bounding_volume();

            if !a.value.borrow().bounding_volume.contains(&new_bv) {
                // need an update!
                new_bv.loosen(self.margin.clone());

                {
                    let mut bv = a.value.borrow_mut();
                    bv.bounding_volume = new_bv;
                }

                self.tree.remove(&mut a.value);
                self.to_update.push(a.value.clone());
            }
        }

        self.update_updatable();
    }

    fn update_object(&mut self, object: &B) {
        match self.active2bv.find_mut(&object.uid()) {
            None       => { },
            Some(leaf) => {
                let mut new_bv = leaf.borrow().object.bounding_volume();
                if !leaf.borrow().bounding_volume.contains(&new_bv) {
                    // update for real
                    new_bv.loosen(self.margin.clone());
                    {
                        let mut bl = leaf.borrow_mut();
                        bl.bounding_volume = new_bv;
                    }
                    self.tree.remove(leaf);
                    self.to_update.push(leaf.clone());
                }
            }
        }

        self.update_updatable();
    }

    #[inline(always)]
    fn for_each_pair(&self, f: |&B, &B, &DV| -> ()) {
        for p in self.pairs.elements().iter() {
            let bf = p.key.first.borrow_mut();
            let bs = p.key.second.borrow_mut();
            f(&bf.object, &bs.object, &p.value)
        }
    }

    #[inline(always)]
    fn for_each_pair_mut(&mut self, f: |&B, &B, &mut DV| -> ()) {
        for p in self.pairs.elements_mut().iter_mut() {
            let bf = p.key.first.borrow();
            let bs = p.key.second.borrow();
            f(&bf.object, &bs.object, &mut p.value)
        }
    }

    #[inline(always)]
    fn activate(&mut self, body: &B, f: |&B, &B, &mut DV| -> ()) {
        // verify that it is not already active and add it to the active map.
        let mut leaf =
            match self.inactive2bv.get_and_remove(&body.uid()) {
                None    => return, // not found: the object is already active
                Some(l) => l.value
            };

        self.active2bv.insert(body.uid(), leaf.clone());

        // remove from the inactive tree
        self.stree.remove(&mut leaf);

        // Now we find interferences with inactive objects.
        { // scope to avoid dynamic borrow failure
            let leaf = leaf.clone();
            let bleaf = leaf.borrow();
            self.stree.interferences_with_leaf(bleaf.deref(), &mut self.collector);

            for i in self.collector.iter() {
                let bi = i.borrow();
                if self.dispatcher.is_valid(&bleaf.object, &bi.object) {
                    // the intereference should be registered on the spairs already
                    match self.spairs.get_and_remove(&Pair::new(leaf.clone(), i.clone())) {
                        Some(dv) => {
                            let key   = dv.key.clone();
                            let value = dv.value;
                            let bdvf  = key.first.borrow();
                            let bdvs  = key.second.borrow();
                            let key   = dv.key;
                            let obj1  = &bdvf.object;
                            let obj2  = &bdvs.object;
                            let p     = self.pairs.insert_or_replace(key, value, true);

                            f(obj1, obj2, p)
                        },
                        None => panic!("Internal error: found a new collision during the activation.")
                    }
                }
            }
        }

        // add to the active tree
        self.tree.insert(leaf);
        self.collector.clear();
    }

    fn deactivate(&mut self, body: &B) {
        // verify that it is not already inactive and add it to the inactive map.
        let mut leaf =
            match self.active2bv.get_and_remove(&body.uid()) {
                None    => return, // not found: the object is already inactive
                Some(l) => l.value
            };

        self.inactive2bv.insert(body.uid(), leaf.clone());

        // Now transfer all collisions involving `leaf` and deactivated objects from `pairs` to
        // `spairs`.

        // remove from the active tree
        self.tree.remove(&mut leaf);

        { // scope to avoid dynamic borrow failure of leaf
            let cleaf = leaf.clone();
            let bleaf = cleaf.borrow();
            self.stree.interferences_with_leaf(bleaf.deref(), &mut self.collector);

            for i in self.collector.iter() {
                let fi = i.borrow();
                if self.dispatcher.is_valid(&bleaf.object, &fi.object) {
                    // the intereference should be registered on the pairs already
                    match self.pairs.get_and_remove(&Pair::new(leaf.clone(), i.clone())) {
                        Some(dv) => { self.spairs.insert(dv.key, dv.value); },
                        None     => panic!("Internal error: found a new collision during the deactivation.")
                    }
                }
            }
        }

        // add to the inactive tree
        self.stree.insert(leaf);
        self.collector.clear();
    }

    fn interferences_with_bounding_volume(&mut self, bv: &BV, out: &mut Vec<B>) {
        {
            let mut visitor = BoundingVolumeInterferencesCollector::new(bv, &mut self.collector);

            self.tree.visit(&mut visitor);
            self.stree.visit(&mut visitor);
        }

        for l in self.collector.iter() {
            out.push(l.borrow().object.clone())
        }

        self.collector.clear()
    }

    fn interferences_with_ray(&mut self, ray: &Ray<P, V>, out: &mut Vec<B>) {
        {
            let mut visitor = RayInterferencesCollector::new(ray, &mut self.collector);

            self.tree.visit(&mut visitor);
            self.stree.visit(&mut visitor);
        }

        for l in self.collector.iter() {
            out.push(l.borrow().object.clone())
        }

        self.collector.clear()
    }

    fn interferences_with_point(&mut self, point: &P, out: &mut Vec<B>) {
        {
            let mut visitor = PointInterferencesCollector::new(point, &mut self.collector);

            self.tree.visit(&mut visitor);
            self.stree.visit(&mut visitor);
        }

        for l in self.collector.iter() {
            out.push(l.borrow().object.clone())
        }

        self.collector.clear()
    }
}
