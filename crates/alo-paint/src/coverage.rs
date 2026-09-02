//! How much of each pixel a shape covers.
//!
//! **Coverage is not colour.** A rasterised shape says how much of each pixel
//! it covers, from zero to 255; the colour arrives when the coverage is
//! composited. That is what lets one glyph mask serve black text on white and
//! white text on black, and it is what makes a shadow cheap: a shadow is the
//! same mask, moved and blurred, drawn in a different colour.
//!
//! The type is here rather than beside the rasteriser because more than one
//! thing makes coverage — [`crate::raster::fill`] from a path,
//! [`crate::blur::blurred`] from other coverage — and the thing they have in
//! common should not belong to either of them.

/// How much of each pixel a shape covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    width: u32,
    height: u32,
    /// The left and top of the covered area, in whole pixels, relative to
    /// wherever the path's own coordinates were.
    origin: (i32, i32),
    data: Vec<u8>,
}

impl Coverage {
    /// Nothing covered at all.
    pub fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            origin: (0, 0),
            data: Vec::new(),
        }
    }

    /// How wide the covered area is, in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// How tall it is.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Where its top-left corner is, in whole pixels.
    ///
    /// A glyph is outlined with the pen at the origin, so this is usually
    /// negative in `y`: the ink starts above the baseline.
    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    /// Whether nothing is covered.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// How much of one pixel is covered, from zero to 255.
    ///
    /// Coordinates are relative to [`Coverage::origin`]. A pixel outside the
    /// covered area is covered not at all, which is a real answer rather than
    /// an error.
    pub fn at(&self, x: u32, y: u32) -> u8 {
        if x >= self.width || y >= self.height {
            return 0;
        }
        let index = (y as usize) * (self.width as usize) + (x as usize);
        self.data.get(index).copied().unwrap_or(0)
    }

    /// Every pixel's coverage, row by row from the top.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Coverage {
    /// Coverage from values already worked out, row by row from the top.
    ///
    /// Anything whose length does not match the size it claims comes back
    /// empty rather than reading past its own data.
    pub fn new(width: u32, height: u32, origin: (i32, i32), data: Vec<u8>) -> Self {
        let expected = (width as usize).saturating_mul(height as usize);
        if data.len() != expected || expected == 0 {
            return Self::empty();
        }
        Self {
            width,
            height,
            origin,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_covers_nothing() {
        let coverage = Coverage::empty();
        assert!(coverage.is_empty());
        assert_eq!((coverage.width(), coverage.height()), (0, 0));
        assert_eq!(coverage.origin(), (0, 0));
        assert_eq!(coverage.at(0, 0), 0);
        assert!(coverage.data().is_empty());
    }

    #[test]
    fn coverage_reads_back_the_values_it_was_made_with() {
        let coverage = Coverage::new(2, 2, (3, -4), vec![0, 64, 128, 255]);
        assert_eq!(coverage.origin(), (3, -4));
        assert_eq!(coverage.at(0, 0), 0);
        assert_eq!(coverage.at(1, 0), 64);
        assert_eq!(coverage.at(0, 1), 128);
        assert_eq!(coverage.at(1, 1), 255);
    }

    #[test]
    fn asking_outside_the_covered_area_is_answered_with_nothing() {
        let coverage = Coverage::new(2, 2, (0, 0), vec![255; 4]);
        assert_eq!(coverage.at(2, 0), 0);
        assert_eq!(coverage.at(0, 2), 0);
        assert_eq!(coverage.at(u32::MAX, u32::MAX), 0);
    }

    #[test]
    fn values_that_do_not_match_the_size_they_claim_are_refused() {
        assert!(Coverage::new(4, 4, (0, 0), vec![255; 3]).is_empty());
        assert!(Coverage::new(0, 4, (0, 0), Vec::new()).is_empty());
    }
}
