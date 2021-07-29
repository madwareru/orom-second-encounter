use simple_tiled_wfc::grid_generation::{WfcEntropyHeuristic, WfcModule, DefaultEntropyChoiceHeuristic, WfcEntropyChoiceHeuristic};
use bitsetium::{BitSearch, BitEmpty, BitSet, BitIntersection, BitUnion, BitTestNone};
use std::hash::Hash;
use simple_tiled_wfc::{get_bits_set_count, BitsIterator};
use rand::{thread_rng, Rng};

pub fn square_dist(x1: usize, y1: usize, x2: usize, y2: usize) -> f64 {
    (x2 as f64 - x1 as f64) * (x2 as f64 - x1 as f64) +
        (y2 as f64 - y1 as f64) * (y2 as f64 - y1 as f64)
}

pub struct LeastDistanceHeuristic {
    pub row: usize,
    pub column: usize,
}

impl<TBitSet> WfcEntropyHeuristic<TBitSet> for LeastDistanceHeuristic
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    fn choose_next_collapsed_slot(
        &self,
        width: usize,
        _height: usize,
        _modules: &[WfcModule<TBitSet>],
        available_indices: &[usize]
    ) -> usize {
        let (mut min_id, mut min_distance) = (available_indices.len() - 1, f64::MAX);
        for i in 0..available_indices.len() {
            let idx = available_indices[i];
            let row = idx / width;
            let column = idx % width;
            let d = square_dist(self.row, self.column, row, column);
            if d < min_distance {
                min_id = i;
                min_distance = d;
            }
        }
        min_id
    }
}

pub struct DrawingChoiceHeuristic<TBitSet>
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    pub fallback: DefaultEntropyChoiceHeuristic,
    pub preferable_bits: TBitSet
}
impl<TBitSet> WfcEntropyChoiceHeuristic<TBitSet> for DrawingChoiceHeuristic<TBitSet>
    where TBitSet:
    BitSearch + BitEmpty + BitSet + BitIntersection +
    BitUnion + BitTestNone + Hash + Eq + Copy + BitIntersection<Output = TBitSet> +
    BitUnion<Output = TBitSet>
{
    fn choose_least_entropy_bit(
        &self,
        width: usize,
        height: usize,
        row: usize,
        column: usize,
        modules: &[WfcModule<TBitSet>],
        slot_bits: &TBitSet
    ) -> usize {
        let intersection = self.preferable_bits.intersection(*slot_bits);
        if get_bits_set_count(&intersection) > 0 {
            let mut rng = thread_rng();
            let random_bit_id = rng.gen_range(0, get_bits_set_count(&intersection));
            let mut iterator = BitsIterator::new(&intersection);
            iterator.nth(random_bit_id).unwrap()
        } else {
            self.fallback.choose_least_entropy_bit(width, height, row, column, modules, slot_bits)
        }
    }
}