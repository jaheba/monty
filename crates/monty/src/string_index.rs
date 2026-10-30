//! Derived UTF-8 offsets. Caches are discarded on clone and never persisted.
use std::cell::{Cell, RefCell};

use monty_types::ResourceTracker;

use crate::exception_private::RunResult;

const STRIDE: usize = 32;
const SCAN_BUDGET: usize = 32;

/// Belongs to one immutable string; offsets cannot be reused with different text.
#[derive(Debug)]
pub(crate) struct StringIndex {
    // Zero means unknown; every string length is representable after adding one.
    length: Cell<usize>,
    state: RefCell<Option<Box<State>>>,
}

impl Default for StringIndex {
    fn default() -> Self {
        Self {
            length: Cell::new(0),
            state: RefCell::new(None),
        }
    }
}

impl Clone for StringIndex {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
struct State {
    last: (usize, usize),
    scanned: usize,
    offsets: Offsets,
}

#[derive(Debug, Default)]
enum Offsets {
    #[default]
    Empty,
    Compact(Box<[u32]>),
    Wide(Box<[usize]>),
}

impl Offsets {
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Compact(v) => v.len(),
            Self::Wide(v) => v.len(),
        }
    }

    fn get(&self, index: usize) -> usize {
        match self {
            Self::Empty => 0,
            Self::Compact(v) => v[index] as usize,
            Self::Wide(v) => v[index],
        }
    }

    fn anchor(&self, byte: usize) -> (usize, usize) {
        let mut low = 0;
        let mut high = self.len();
        while low < high {
            let mid = low + (high - low) / 2;
            if self.get(mid) <= byte {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        let checkpoint = low.saturating_sub(1);
        (checkpoint * STRIDE, self.get(checkpoint))
    }
}

impl StringIndex {
    pub fn char_len(&self, text: &str) -> usize {
        if let Some(length) = self.length.get().checked_sub(1) {
            length
        } else {
            let length = text.chars().count();
            self.length.set(length + 1);
            length
        }
    }

    pub fn character(&self, text: &str, index: i64, tracker: &ResourceTracker) -> RunResult<Option<char>> {
        // Endpoints neither allocate a cursor nor trigger an index build.
        if index == 0 {
            return Ok(text.chars().next());
        }
        if index == -1 {
            return Ok(text.chars().next_back());
        }
        let target = if index < 0 {
            let Ok(distance) = usize::try_from(index.unsigned_abs()) else {
                return Ok(None);
            };
            let Some(target) = self.char_len(text).checked_sub(distance) else {
                return Ok(None);
            };
            target
        } else {
            let Ok(target) = usize::try_from(index) else {
                return Ok(None);
            };
            target
        };
        let byte = self.byte_offset(text, target, tracker)?;
        Ok(text[byte..].chars().next())
    }

    /// Maps a character boundary to bytes, clamping beyond the end like slicing.
    pub fn byte_offset(&self, text: &str, target: usize, tracker: &ResourceTracker) -> RunResult<usize> {
        if target == 0 {
            return Ok(0);
        }
        if target == usize::MAX || self.length.get().checked_sub(1).is_some_and(|length| target >= length) {
            return Ok(text.len());
        }
        let mut state = self.state.borrow_mut();
        let state = state.get_or_insert_with(Box::default);
        let checkpoint = target / STRIDE;
        let (mut character, mut byte) = if checkpoint < state.offsets.len() {
            (checkpoint * STRIDE, state.offsets.get(checkpoint))
        } else {
            (0, 0)
        };
        let (last_character, last_byte) = state.last;
        if last_character <= target && last_character >= character {
            (character, byte) = state.last;
        } else if last_character > target && last_character - target <= 8 {
            let mut chars = text[..last_byte].chars();
            chars.nth_back(last_character - target - 1);
            let result = chars.as_str().len();
            state.last = (target, result);
            return Ok(result);
        }
        if target == character {
            return Ok(byte);
        }
        let mut chars = text[byte..].chars();
        if chars.nth(target - character - 1).is_none() {
            // Remember the length so repeated invalid accesses do not rescan.
            self.char_len(text);
            return Ok(text.len());
        }
        let result = text.len() - chars.as_str().len();
        state.last = (target, result);
        self.charge(text, state, target - character, result - byte, tracker)?;
        Ok(result)
    }

    /// Converts a known UTF-8 boundary returned by substring matching.
    pub fn char_offset(&self, text: &str, target: usize, tracker: &ResourceTracker) -> RunResult<usize> {
        if target == 0 {
            return Ok(0);
        }
        if target == text.len() {
            return Ok(self.char_len(text));
        }
        let mut state = self.state.borrow_mut();
        let state = state.get_or_insert_with(Box::default);
        let (mut character, mut byte) = state.offsets.anchor(target);
        if state.last.1 <= target && state.last.1 >= byte {
            (character, byte) = state.last;
        }
        let traversed = text[byte..target].chars().count();
        let result = character + traversed;
        state.last = (result, target);
        self.charge(text, state, traversed, target - byte, tracker)?;
        Ok(result)
    }

    fn charge(
        &self,
        text: &str,
        state: &mut State,
        characters: usize,
        bytes: usize,
        tracker: &ResourceTracker,
    ) -> RunResult<()> {
        if state.offsets.len() != 0 || characters <= 8 {
            return Ok(());
        }
        state.scanned = state.scanned.saturating_add(bytes);
        if state.scanned < text.len().saturating_mul(SCAN_BUDGET) {
            return Ok(());
        }
        let count = self.char_len(text).div_ceil(STRIDE);
        if u32::try_from(text.len()).is_ok() {
            tracker.check_allocation(count.saturating_mul(size_of::<u32>()))?;
            let mut offsets = Vec::with_capacity(count);
            for (character, (byte, _)) in text.char_indices().enumerate() {
                if character % STRIDE == 0 {
                    offsets.push(u32::try_from(byte).expect("compact string offset"));
                }
            }
            state.offsets = Offsets::Compact(offsets.into_boxed_slice());
        } else {
            tracker.check_allocation(count.saturating_mul(size_of::<usize>()))?;
            let mut offsets = Vec::with_capacity(count);
            offsets.extend(text.char_indices().step_by(STRIDE).map(|(byte, _)| byte));
            state.offsets = Offsets::Wide(offsets.into_boxed_slice());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use monty_types::ResourceLimits;

    #[test]
    fn random_local_and_inverse_offsets() {
        let tracker = ResourceTracker::default();
        for text in ["", "ascii", "aé中🐍", "abcdefghüijk中🐍"].map(|s| s.repeat(300)) {
            let index = StringIndex::default();
            let characters: Vec<_> = text.chars().collect();
            let boundaries: Vec<_> = text.char_indices().map(|(byte, _)| byte).chain([text.len()]).collect();
            for n in 0..4096 {
                let target = n * 7919 % (characters.len() + 1);
                assert_eq!(index.byte_offset(&text, target, &tracker).unwrap(), boundaries[target]);
                assert_eq!(index.char_offset(&text, boundaries[target], &tracker).unwrap(), target);
                assert_eq!(
                    index.character(&text, target as i64, &tracker).unwrap(),
                    characters.get(target).copied()
                );
                let negative = -(target as i64) - 1;
                let expected = characters.len().checked_sub(target + 1).map(|i| characters[i]);
                assert_eq!(index.character(&text, negative, &tracker).unwrap(), expected);
            }
            assert_eq!(index.character(&text, i64::MIN, &tracker).unwrap(), None);
            assert_eq!(index.character(&text, i64::MAX, &tracker).unwrap(), None);
        }
    }

    #[test]
    fn endpoints_and_local_reads_do_not_build_offsets() {
        let tracker = ResourceTracker::default();
        let text = "a中é🐍".repeat(4096);
        let index = StringIndex::default();
        for _ in 0..100 {
            index.character(&text, 0, &tracker).unwrap();
            index.character(&text, -1, &tracker).unwrap();
        }
        assert!(index.state.borrow().is_none());
        for target in 1..text.chars().count() {
            index.character(&text, target as i64, &tracker).unwrap();
        }
        assert_eq!(index.state.borrow().as_ref().unwrap().offsets.len(), 0);
    }

    #[test]
    fn adaptive_index_waits_for_sustained_random_access() {
        let tracker = ResourceTracker::default();
        let text = "a中é🐍".repeat(1024);
        let index = StringIndex::default();
        for n in 0..64 {
            index.character(&text, (n * 7919 % 4096) as i64, &tracker).unwrap();
        }
        assert_eq!(index.state.borrow().as_ref().unwrap().offsets.len(), 0);
        for n in 64..4096 {
            index.character(&text, (n * 7919 % 4096) as i64, &tracker).unwrap();
        }
        assert!(index.state.borrow().as_ref().unwrap().offsets.len() > 0);
    }

    #[test]
    fn allocation_limit_and_clone_reset() {
        let text = "a中é🐍".repeat(4096);
        let index = StringIndex::default();
        let tracker = ResourceTracker::new(ResourceLimits {
            max_memory: Some(0),
            ..ResourceLimits::default()
        });
        // Alternating distant positions exhausts the scan budget.
        let mut rejected = false;
        for n in 0..256 {
            if index
                .character(&text, if n % 2 == 0 { 8000 } else { 1 }, &tracker)
                .is_err()
            {
                rejected = true;
                break;
            }
        }
        assert!(rejected);
        let cloned = index.clone();
        assert_eq!(cloned.length.get(), 0);
        assert!(cloned.state.borrow().is_none());
    }
}
