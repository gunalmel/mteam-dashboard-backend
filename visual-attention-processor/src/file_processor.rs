use std::io::Read;
use mteam_dashboard_utils::json::parse_json_array_root;
use std::collections::HashMap;
use mteam_dashboard_utils::date_parser::seconds_to_csv_row_time;
use crate::data_point_parser;

pub fn process_visual_attention_data(reader: &mut impl Read, window_duration_secs: u32)  -> Result<impl Iterator<Item = (String, String, f64)>, String>{
    normalize_visual_attention_load_data(reader).and_then(|normalized_data_iter| {
        Ok(aggregate_category_ratios(normalized_data_iter, window_duration_secs))
    })
}

pub fn normalize_visual_attention_load_data(reader: &mut impl Read) -> Result<impl Iterator<Item = (f64, Option<String>)>, String> {
    let root_array = parse_json_array_root(reader)?;

    Ok(root_array.into_iter().scan(None, |state, item| {
        let mapped_time =
            data_point_parser::map_time_to_date(item, *state).map(|(date_time, cognitive_load, first_timestamp)| {
                *state = first_timestamp;
                (date_time, cognitive_load)
            });

        mapped_time
    }))
}

pub fn aggregate_category_ratios(data_iter: impl Iterator<Item = (f64, Option<String>)>, window_size: u32) -> impl Iterator<Item = (String, String, f64)> {
    let sliding_window = SlidingWindow {
        data_iter,
        window_size,
        window_start: 0,
        window_end: window_size,
        category_count: Default::default(),
        total_count: 0,
    };

    sliding_window.flat_map(|results| results.into_iter())
}

struct SlidingWindow<I: Iterator<Item = (f64, Option<String>)>> {
    data_iter: I,
    window_start: u32,
    window_end: u32,
    window_size: u32,
    category_count: HashMap<String, usize>,
    total_count: usize,
}

impl<I: Iterator<Item = (f64, Option<String>)>> Iterator for SlidingWindow<I> {
    type Item = Vec<(String, String, f64)>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut results = Vec::new();
        while let Some((time, category)) = self.data_iter.next() {
            if time > self.window_end as f64 {
                for (cat, count) in self.category_count.drain() {
                    let window_end_date = seconds_to_csv_row_time(self.window_end).date_string;
                    results.push((cat, window_end_date, count as f64 / self.total_count as f64));
                }
                self.window_start = self.window_end;
                self.window_end += self.window_size;
                self.total_count = 0;
                self.category_count.clear();
            }

            if let Some(cat) = category {
                *self.category_count.entry(cat.clone()).or_insert(0) += 1;
                self.total_count += 1;
            }
        }

        if self.total_count > 0 {
            for (cat, count) in self.category_count.drain() {
                let window_end_date = seconds_to_csv_row_time(self.window_end).date_string;
                results.push((cat, window_end_date, count as f64 / self.total_count as f64));
            }
        }

        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }
}