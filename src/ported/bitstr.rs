#[repr(transparent)]
pub struct BitStr {
    bits: Box<[u8]>,
}

impl BitStr {
    pub fn new(nbits: u32) -> Self {
        Self {
            bits: vec![0; nbits.div_ceil(8) as usize].into_boxed_slice()
        }
    }

    pub fn bit_set(&mut self, i: u32) {
        let byte_index = i / 8;
        let bit_index = i % 8;
        self.bits[byte_index as usize] |= 1 << bit_index;
    }

    #[inline]
    pub fn bit_clear(&mut self, i: u32) {
        let byte_index = i / 8;
        let bit_index = i % 8;
        self.bits[byte_index as usize] &= !(1 << bit_index); 
    }

    pub fn bit_nclear(&mut self, start: u32, stop: u32) {
        // TODO this is written inefficiently, assuming the compiler will optimize it. if it doesn't rewrite it
        for i in start..=stop {
            self.bit_clear(i);
        }
    }

    pub fn bit_test(&self, i: u32) -> bool {
        let byte_index = i / 8;
        let bit_index = i % 8;
        self.bits[byte_index as usize] & (1 << bit_index) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Behavior is derived from vendor/tmux/compat/bitstring.h:
    //   bitstr_size(nbits) = ((nbits) + 7) >> 3   (bytes needed)
    //   bit_alloc/calloc    => all bits start clear (0)
    //   _bit_byte(bit)      = bit >> 3
    //   _bit_mask(bit)      = 1 << (bit & 0x7)
    //   bit_test(name, bit) = name[bit>>3] & (1 << (bit&7))
    //   bit_set(name, bit)  = name[bit>>3] |= (1 << (bit&7))
    //   bit_clear(name, bit)= name[bit>>3] &= ~(1 << (bit&7))
    //   bit_nclear(name, start, stop): clear bits start..=stop inclusive

    // Helper: count how many byte the allocation reserves.
    // bit_decl / bitstr_size == (nbits + 7) >> 3.
    fn expected_size(nbits: u32) -> usize {
        ((nbits + 7) >> 3) as usize
    }

    #[test]
    fn new_allocates_all_bits_clear() {
        // calloc-backed allocation: every bit must read as clear.
        let b = BitStr::new(64);
        for i in 0..64 {
            assert!(!b.bit_test(i), "bit {i} should start clear");
        }
    }

    #[test]
    fn new_reserves_correct_byte_count() {
        // bitstr_size rounds up to whole bytes.
        assert_eq!(BitStr::new(1).bits.len(), expected_size(1)); // 1 byte
        assert_eq!(BitStr::new(8).bits.len(), expected_size(8)); // 1 byte
        assert_eq!(BitStr::new(9).bits.len(), expected_size(9)); // 2 bytes
        assert_eq!(BitStr::new(16).bits.len(), expected_size(16)); // 2 bytes
        assert_eq!(BitStr::new(17).bits.len(), expected_size(17)); // 3 bytes
        assert_eq!(BitStr::new(1).bits.len(), 1);
        assert_eq!(BitStr::new(9).bits.len(), 2);
        assert_eq!(BitStr::new(17).bits.len(), 3);
    }

    #[test]
    fn set_then_test() {
        let mut b = BitStr::new(64);
        // Set a spread of bits across byte boundaries.
        for &i in &[0u32, 1, 7, 8, 15, 16, 31, 32, 63] {
            b.bit_set(i);
        }
        for i in 0..64u32 {
            let want = [0u32, 1, 7, 8, 15, 16, 31, 32, 63].contains(&i);
            assert_eq!(b.bit_test(i), want, "bit {i}");
        }
    }

    #[test]
    fn set_is_idempotent() {
        // bit_set uses |=, so repeated sets leave the bit set.
        let mut b = BitStr::new(8);
        b.bit_set(3);
        b.bit_set(3);
        assert!(b.bit_test(3));
        // Neighbours are untouched.
        assert!(!b.bit_test(2));
        assert!(!b.bit_test(4));
    }

    #[test]
    fn clear_only_target_bit() {
        // bit_clear uses &= ~mask: only the addressed bit changes.
        let mut b = BitStr::new(8);
        for i in 0..8 {
            b.bit_set(i);
        }
        b.bit_clear(4);
        for i in 0..8u32 {
            assert_eq!(b.bit_test(i), i != 4, "bit {i}");
        }
    }

    #[test]
    fn clear_is_idempotent_on_unset_bit() {
        let mut b = BitStr::new(8);
        assert!(!b.bit_test(2));
        b.bit_clear(2);
        assert!(!b.bit_test(2));
    }

    #[test]
    fn set_clear_roundtrip() {
        let mut b = BitStr::new(32);
        for i in 0..32 {
            b.bit_set(i);
            assert!(b.bit_test(i));
            b.bit_clear(i);
            assert!(!b.bit_test(i));
        }
    }

    #[test]
    fn nclear_inclusive_range() {
        // bit_nclear clears start..=stop inclusive, leaving the rest set.
        let mut b = BitStr::new(32);
        for i in 0..32 {
            b.bit_set(i);
        }
        b.bit_nclear(8, 23);
        for i in 0..32u32 {
            let in_range = (8..=23).contains(&i);
            assert_eq!(b.bit_test(i), !in_range, "bit {i}");
        }
    }

    #[test]
    fn nclear_single_bit_range() {
        // start == stop clears exactly one bit (loop runs once).
        let mut b = BitStr::new(16);
        for i in 0..16 {
            b.bit_set(i);
        }
        b.bit_nclear(5, 5);
        for i in 0..16u32 {
            assert_eq!(b.bit_test(i), i != 5, "bit {i}");
        }
    }

    #[test]
    fn nclear_spanning_byte_boundaries() {
        // Range crossing several bytes: bits 6..=17 span three bytes.
        let mut b = BitStr::new(24);
        for i in 0..24 {
            b.bit_set(i);
        }
        b.bit_nclear(6, 17);
        for i in 0..24u32 {
            let in_range = (6..=17).contains(&i);
            assert_eq!(b.bit_test(i), !in_range, "bit {i}");
        }
    }

    #[test]
    fn nclear_full_range() {
        let mut b = BitStr::new(16);
        for i in 0..16 {
            b.bit_set(i);
        }
        b.bit_nclear(0, 15);
        for i in 0..16u32 {
            assert!(!b.bit_test(i), "bit {i} should be clear");
        }
    }

    #[test]
    fn high_bit_in_byte_does_not_leak() {
        // Setting bit 7 (mask 0x80) must not be seen as bit 0 of next byte.
        let mut b = BitStr::new(16);
        b.bit_set(7);
        assert!(b.bit_test(7));
        assert!(!b.bit_test(8));
        assert!(!b.bit_test(6));
    }
}
