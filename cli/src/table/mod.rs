pub mod ago;

use colored::{ColoredString, Colorize};
use std::cmp;
use terminal_size::{terminal_size, Width};

pub struct Column {
    header: String,
    force: bool,
    centered: bool,
    ellipse: bool,
}

impl Column {
    pub fn new(header: &str) -> Self {
        Self { header: header.into(), force: false, centered: false, ellipse: false }
    }

    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }

    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    pub fn ellipse(mut self) -> Self {
        self.ellipse = true;
        self
    }
}

pub fn table<const COUNT: usize>(
    columns: [Column; COUNT],
    rows: Vec<[ColoredString; COUNT]>,
    sep: &str,
) {
    // don't display empty tables
    if rows.is_empty() {
        return;
    }

    // calculate data for columns
    let data = (0..COUNT)
        .map(|i| {
            let column = &columns[i];
            let elements = rows.iter().map(|row| row[i].len());

            let min = cmp::min(elements.clone().min().unwrap_or(usize::MAX), column.header.len());
            let max = cmp::max(elements.clone().max().unwrap_or(usize::MIN), column.header.len());
            let avg = elements.clone().sum::<usize>() / rows.len();

            // determine forced columns
            let width = if column.force { Some(max) } else { None };

            (column, min, max, avg, width)
        })
        .collect::<Vec<_>>();

    // maximal table width
    let maximal = data.iter().map(|p| p.2).sum::<usize>();

    // width to make table as
    let width = cmp::min(
        terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(usize::MAX)
            - (COUNT - 1) * sep.len(), // terminal size
        maximal, // maximal size of content
    );

    // dynamic columns
    let mut available = width - cmp::min(data.iter().filter_map(|p| p.4).sum::<usize>(), width);
    let mut ratio = data.iter().filter(|p| p.4.is_none()).map(|p| p.3).sum::<usize>();

    let width = data
        .into_iter()
        .map(|(column, _, max, avg, w)| {
            let width = w.unwrap_or_else(|| {
                if width >= maximal {
                    max
                } else {
                    let take = ((avg as f32 / ratio as f32) * available as f32) as usize;

                    available -= take;
                    ratio -= avg;

                    take
                }
            });

            (column, width)
        })
        .collect::<Vec<_>>();

    // header
    let header = width
        .iter()
        .map(|(column, width)| format!("{:<1$.1$}", column.header, width))
        .intersperse(sep.to_string())
        .collect::<String>();

    println!("{}", header.italic());

    // body
    for row in rows {
        let row = row
            .iter()
            .zip(width.iter())
            .map(|(s, (column, width))| match (column.centered, column.ellipse, s.len() > *width) {
                (_, true, true) => format!("{s:<0$.0$}...", width - 3),
                (true, _, _) => format!("{s:^0$.0$}", width),
                (_, _, _) => {
                    format!("{s:<0$.0$}", width)
                }
            })
            .intersperse(sep.to_string())
            .collect::<String>();

        println!("{}", row);
    }
}
