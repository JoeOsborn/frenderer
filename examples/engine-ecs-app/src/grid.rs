pub type Coord = (usize, usize);

#[allow(dead_code)]
pub struct Grid<T> {
    width: usize,
    height: usize,
    storage: Box<[T]>,
}

// // If you want to get fancy, you can use this definition of grid that
// // fixes a constant width and height at compile time:
// struct Grid<T, const W: usize, const H: usize> {
//     storage: Box<[[T; W]; H]>,
// }

#[allow(dead_code)]
impl<T> Grid<T> {
    pub fn new(width: usize, height: usize, cells: impl IntoIterator<Item = T>) -> Self {
        let cells: Vec<T> = cells.into_iter().collect();
        assert_eq!(
            cells.len(),
            width * height,
            "Not the right number of cells for the given width and height"
        );
        Self {
            width,
            height,
            storage: cells.into_boxed_slice(),
        }
    }
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn row_iter(&self) -> impl Iterator<Item = &[T]> {
        self.storage.chunks(self.width)
    }
    pub fn get_index(&self, idx: usize) -> Option<&T> {
        self.storage.get(idx)
    }
    pub fn get_index_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.storage.get_mut(idx)
    }
    pub fn get(&self, x: usize, y: usize) -> Option<&T> {
        self.storage.get(self.xy_to_index(x, y)?)
    }
    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut T> {
        self.storage.get_mut(self.xy_to_index(x, y)?)
    }
    pub fn xy_to_index(&self, x: usize, y: usize) -> Option<usize> {
        if self.contains(x, y) {
            //todo!("Try implementing xy_to_index based on index_to_coord!")
            Some(y * self.width + x)
        } else {
            None
        }
    }
    pub fn coord_to_index(&self, (x, y): Coord) -> Option<usize> {
        self.xy_to_index(x, y)
    }
    pub fn index_to_coord(&self, idx: usize) -> Option<Coord> {
        if idx < self.storage.len() {
            Some((idx % self.width, idx / self.width))
        } else {
            None
        }
    }
    pub fn contains(&self, x: usize, y: usize) -> bool {
        // Don't just use self.get(x,y).is_some()!  It uses contains to avoid invalid coordinates.
        //todo!("Implement a function to test if x,y is in bounds")
        x < self.width && y < self.height
    }
    // This will return an iterator so we don't commit to a particular
    // number of neighbors (e.g. if they are out of bounds).  We can't
    // just return four and hope for the best, because the neighbors
    // of 0,0 include some negative coordinates which can't even be
    // represented as usizes.  We could return a boxed slice, but that
    // would mean a heap allocation when we can probably get away with
    // just stack usage.  We could also use a struct like {
    // storage:[Coord;4], amt:u8 } that has some blank or unused
    // entries in the storage array (anything more than amt would be
    // "unused").  Using an iterator keeps things as simple as we can.
    pub fn neighbors_4(&self, x: usize, y: usize) -> impl Iterator<Item = Coord> {
        // We'll use
        let left = x.checked_sub(1);
        let right = x.checked_add(1);
        let above = y.checked_sub(1);
        let below = y.checked_add(1);
        let w = self.width;
        let h = self.height;
        [
            (left, Some(y)),
            (Some(x), above),
            (right, Some(y)),
            (Some(x), below),
        ]
        .into_iter()
        .filter_map(move |(x, y)| {
            // Unfortunately we can't use self.contains() here or the
            // iterator is bound up in the grid's lifetime.
            // and_then produces an option from the inner value.
            x.zip(y)
                .and_then(|(x, y)| if x < w && y < h { Some((x, y)) } else { None })
        })
    }
    pub fn neighbors_8(&self, x: usize, y: usize) -> impl Iterator<Item = Coord> {
        //todo!("following the example above, what are the eight neighbors of this tile?")
        let left = x.checked_sub(1);
        let right = x.checked_add(1);
        let above = y.checked_sub(1);
        let below = y.checked_add(1);
        let w = self.width;
        let h = self.height;
        [
            (left, Some(y)),
            (left, above),
            (Some(x), above),
            (right, above),
            (right, Some(y)),
            (left, below),
            (Some(x), below),
            (right, below),
        ]
        .into_iter()
        .filter_map(move |(x, y)| {
            x.zip(y)
                .and_then(|(x, y)| if x < w && y < h { Some((x, y)) } else { None })
        })
    }
}

impl<T> std::ops::Index<usize> for Grid<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get_index(index).unwrap()
    }
}
impl<T> std::ops::IndexMut<usize> for Grid<T> {
    fn index_mut(&mut self, index: usize) -> &mut <Self as std::ops::Index<usize>>::Output {
        self.get_index_mut(index).unwrap()
    }
}
impl<T> std::ops::Index<Coord> for Grid<T> {
    type Output = T;
    fn index(&self, (x, y): Coord) -> &Self::Output {
        self.get(x, y).unwrap()
    }
}
impl<T> std::ops::IndexMut<Coord> for Grid<T> {
    fn index_mut(&mut self, (x, y): Coord) -> &mut <Self as std::ops::Index<Coord>>::Output {
        self.get_mut(x, y).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_grid_coords() {
        let grid = Grid::new(64, 32, vec![0; 64 * 32]);
        for y in 0..32 {
            for x in 0..64 {
                assert_eq!(
                    (x, y),
                    grid.index_to_coord(grid.xy_to_index(x, y).unwrap())
                        .unwrap(),
                    "Invalid coord conversion"
                );
                assert!(grid.xy_to_index(x, y).unwrap() <= 32 * 64);
            }
        }
        for idx in 0..(32 * 64) {
            assert_eq!(
                idx,
                grid.coord_to_index(grid.index_to_coord(idx).unwrap())
                    .unwrap(),
                "Invalid index conversion"
            );
            let (x, y) = grid.index_to_coord(idx).unwrap();
            assert!(x <= 64);
            assert!(y <= 32);
        }
    }
    #[test]
    fn test_neighbors() {
        let grid = Grid::new(64, 32, vec![0; 64 * 32]);
        assert_eq!(grid.neighbors_4(0, 0).count(), 2);
        assert_eq!(grid.neighbors_8(0, 0).count(), 3);
        assert_eq!(grid.neighbors_4(0, 1).count(), 3);
        assert_eq!(grid.neighbors_8(0, 1).count(), 5);
        assert_eq!(grid.neighbors_4(1, 0).count(), 3);
        assert_eq!(grid.neighbors_8(1, 0).count(), 5);
        assert_eq!(grid.neighbors_4(1, 1).count(), 4);
        assert_eq!(grid.neighbors_8(1, 1).count(), 8);
        assert_eq!(grid.neighbors_4(63, 31).count(), 2);
        assert_eq!(grid.neighbors_8(63, 31).count(), 3);
        assert_eq!(grid.neighbors_4(63, 30).count(), 3);
        assert_eq!(grid.neighbors_8(63, 30).count(), 5);
        assert_eq!(grid.neighbors_4(62, 31).count(), 3);
        assert_eq!(grid.neighbors_8(62, 31).count(), 5);
        for y in 0..32 {
            for x in 0..64 {
                for (x, y) in grid.neighbors_4(x, y) {
                    assert!(grid.contains(x, y));
                    assert!(grid.get(x, y).is_some());
                }
                for (x, y) in grid.neighbors_8(x, y) {
                    assert!(grid.contains(x, y));
                    assert!(grid.get(x, y).is_some());
                }
            }
        }
    }
}
