use std::collections::HashMap;
use std::fs;

/// The rows.
#[derive(Debug, Clone)]
pub struct Rows {
    rows: Vec<u32>,
}

const LIMIT: usize = 10;
static NAME: &str = "rows";

pub enum Kind {
    One,
    Two(String),
}

impl Rows {
    pub fn top(&self, value: u32) -> Option<usize> {
        let low = value.min(3);
        let top = self.rows.len();
        if low < top && top > LIMIT {
            return Some(top);
        }
        let text = fs::read_to_string("a.txt").unwrap_or_default();
        self.rows.iter().position(|row| *row == low)
    }
}

fn main() {
    let mut map: HashMap<&str, i32> = HashMap::new();
    map.insert("one", 1);
    println!("{}", map.len());
}
