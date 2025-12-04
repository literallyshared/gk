use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct Cell(pub (i32, i32));

pub struct Grid<T: Copy> {
    cell_size: f32,
    cells: HashMap<Cell, Vec<T>>,
}

impl<T: Copy> Grid<T> {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::default(),
        }
    }

    fn cell_of(&self, x: f32, y: f32) -> Cell {
        Cell(((x / self.cell_size).floor() as i32, (y / self.cell_size).floor() as i32))
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn insert(&mut self, t: T, x: f32, y: f32, w: f32, h: f32) {
        let min = self.cell_of(x - w / 2.0, y - h / 2.0);
        let max = self.cell_of(x + w / 2.0 - f32::EPSILON, y + h / 2.0 - f32::EPSILON);
        for x in min.0.0..=max.0.0 {
            for y in min.0.1..=max.0.1 {
                self.cells.entry(Cell((x, y))).or_default().push(t);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn insert_populates_expected_cells() {
        let mut grid: Grid<u32> = Grid::new(1.0);
        grid.insert(1, 0.0, 0.0, 2.0, 2.0);
        assert_eq!(grid.cells.len(), 4);
        for cell in [(-1, -1), (-1, 0), (0, -1), (0, 0)] {
            assert_eq!(grid.cells.get(&Cell(cell)).unwrap().as_slice(), &[1]);
        }
    }

    #[test]
    fn insert_negative_coords_floor_correctly() {
        let mut grid: Grid<u32> = Grid::new(1.0);
        grid.insert(2, -0.25, -0.25, 0.5, 0.5);
        assert_eq!(grid.cells.len(), 1);
        assert_eq!(grid.cells.get(&Cell((-1, -1))).unwrap().as_slice(), &[2]);
    }
}
