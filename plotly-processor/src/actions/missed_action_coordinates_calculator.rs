use chrono::{Timelike, Utc};
use std::collections::HashMap;

pub struct Rectangle {
    pub name: String,
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
}

/// Calculate the number of points on each row.
///
/// Given the total number of points and the maximum number of points per row,
/// this function returns a vector where each element represents the number of
/// points on a row.
///
/// The points are distributed evenly across the rows, with the last row
/// potentially having fewer points.
pub fn calculate_points_per_row(total_points: u16, max_points_per_row: u16) -> Vec<u16> {
    let mut points_per_row = Vec::new();
    let mut remaining_points = total_points;

    while remaining_points > 0 {
        let points_on_this_row = std::cmp::min(max_points_per_row, remaining_points);
        points_per_row.push(points_on_this_row);
        remaining_points -= points_on_this_row;
    }

    points_per_row
}

/// Calculate the gaps between each point within a line.
///
/// Given the length of the line and the number of points on the line,
/// this function returns the gap between each point. The points are
/// centered within the line, with the first point being `gap` distance
/// from the start of the line and the last point being `gap` distance
/// from the end of the line.
pub fn calculate_gaps_between_points_within_line(line_length: f32, points_on_line: u16) -> f32 {
    if points_on_line == 1 {
        line_length / 2.0
    } else {
        (line_length / (points_on_line as f32 + 1.0)*100.0).ceil()/100.0
    }
}

/// Calculate the coordinates of points within a rectangle.
///
/// Given a rectangle and the number of points per row, this function
/// returns a vector of (date_time, y) coordinates for each point.
pub fn calculate_point_coordinates(rectangle: &Rectangle, points_per_row: &Vec<u16>) -> Vec<(String, f32)> {
    let mut point_coordinates = Vec::new();
    let y_gap = calculate_gaps_between_points_within_line(rectangle.y1 - rectangle.y0, points_per_row.len() as u16);

    let mut y = rectangle.y0 + y_gap;
    for points_on_row in points_per_row {
        let x_gap = calculate_gaps_between_points_within_line(rectangle.x1 - rectangle.x0, *points_on_row);
        let mut x = rectangle.x0 + x_gap;
        for _ in 0..*points_on_row {
            let date_time_str = seconds_to_date_time_string(x);
            point_coordinates.push((date_time_str, y));
            x += x_gap;
        }
        y += y_gap;
    }

    point_coordinates
}

pub struct MissedActionsCoordinatesIterator<'a> {
    hover_text: &'a Vec<String>,
    stages: &'a Vec<(u32, String)>,
    rectangle_map: &'a HashMap<String, Rectangle>,
    rectangle_point_counts: &'a HashMap<String, u16>,
    max_points_per_row: usize,
    current_index: usize,
    current_stage_index: usize,
    current_stage_name: &'a str,
    current_stage_points_coordinates: Vec<(String, f32)>
}

impl<'a> MissedActionsCoordinatesIterator<'a> {
    pub fn new(
        hover_text: &'a Vec<String>,
        stages: &'a Vec<(u32, String)>,
        rectangle_map: &'a HashMap<String, Rectangle>,
        rectangle_point_counts: &'a HashMap<String, u16>,
        max_points_per_row: usize
    ) -> Self {
        assert_eq!(
            hover_text.len(),
            stages.len(),
            "All missed actions should have assigned stages"
        );
        let counts_total = rectangle_point_counts.iter().fold(0, |mut total, (_, count)| {
            total+=count;
            total
        }) as usize;
        assert_eq!(
            hover_text.len(),
            counts_total,
            "All missed actions count in stages should add up to total number of missed actions"
        );
        Self {
            hover_text,
            stages,
            rectangle_map,
            rectangle_point_counts,
            current_index: 0,
            current_stage_index: 0,
            max_points_per_row,
            current_stage_name: "",
            current_stage_points_coordinates: Vec::new()
        }
    }
}

impl<'a> Iterator for MissedActionsCoordinatesIterator<'a> {
    type Item = (String, String, f32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.hover_text.len() {
            return None;
        }
        
        let rectangle_name = &self.stages[self.current_index];
        let rectangle = &self.rectangle_map.get(&rectangle_name.1).unwrap();
        let point_count = *self.rectangle_point_counts.get(&rectangle_name.1).unwrap();
        
        if self.current_stage_name != self.stages[self.current_index].1 {
            let points_per_row = calculate_points_per_row(point_count, self.max_points_per_row as u16);

            self.current_stage_points_coordinates = calculate_point_coordinates(&rectangle, &points_per_row);
            self.current_stage_index=0;
        }
        
        let x = self.current_stage_points_coordinates[self.current_stage_index].0.clone();
        let y = self.current_stage_points_coordinates[self.current_stage_index].1.clone();
        self.current_stage_index+=1;
        
        self.current_stage_name = &self.stages[self.current_index].1;
        let hover_text = &self.hover_text[self.current_index].clone();
        self.current_index += 1;
        Some((hover_text.clone(), x, y))
    }
}

pub fn seconds_to_date_time_string(seconds: f32) -> String {
    let now = Utc::now();
    let today_start = now.with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
    let date_time = today_start + chrono::Duration::seconds(seconds as i64);
    date_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests_calculate_missed_actions_stage_coordinates {
    use crate::config::plotly_mappings::MissedActionsPlotSettings;

    #[test]
    fn missed_actions_stage_y1() {
       let missed_action_plot_settings = MissedActionsPlotSettings {
            max_count_per_row: 3,
            y_increment: -2.0,
            y_min: -0.5
        };
        
        let max_actions_per_row = 7;
        let expected_missed_actions_y_axis_max =  missed_action_plot_settings.y_min + ( max_actions_per_row as f32 / (missed_action_plot_settings.max_count_per_row as f32 + 1.0))   * missed_action_plot_settings.y_increment as f32;

        assert_eq!(expected_missed_actions_y_axis_max, missed_action_plot_settings.calculate_y_max(max_actions_per_row));
        println!("{:?}", expected_missed_actions_y_axis_max);
    }
}
#[cfg(test)]
mod tests_missed_actions_coordinates_iterator {
    const MAX_POINTS_PER_ROW: usize = 2;
    use super::*;
    
    fn create_rectangle_map() -> HashMap<String, Rectangle> {
        HashMap::from([
            ("stageA".to_owned(), Rectangle { x0: 0.0, x1: 100.0, y0: 0.0, y1: -4.0, name: "stageA".to_owned() }),
            ("stageB".to_owned(), Rectangle { x0: 100.0, x1: 200.0, y0: 0.0, y1: -4.0, name: "stageB".to_owned() }),
            ("stageC".to_owned(), Rectangle { x0: 200.0, x1: 300.0, y0: 0.0, y1: -4.0, name: "stageC".to_owned() }),
        ])
    }
    #[test]
    fn single_point_per_row() {
        let hover_text = vec!["text1".to_owned(), "text2".to_owned(), "text3".to_owned()];
        let rectangle_map = create_rectangle_map();
        let stages = vec![(1, "stageA".to_owned()),(2, "stageB".to_owned()),(3, "stageC".to_owned())];
        let mut rectangle_point_counts = HashMap::new();
        rectangle_point_counts.insert("stageA".to_owned(), 1);
        rectangle_point_counts.insert("stageB".to_owned(), 1);
        rectangle_point_counts.insert("stageC".to_owned(), 1);

        let iterator = MissedActionsCoordinatesIterator::new(&hover_text, &stages, &rectangle_map, &rectangle_point_counts, MAX_POINTS_PER_ROW);

        let expected_coordinates = vec![
            ("text1".to_owned(), seconds_to_date_time_string(50.0), -2.0),
            ("text2".to_owned(), seconds_to_date_time_string(150.0), -2.0),
            ("text3".to_owned(), seconds_to_date_time_string(250.0), -2.0),
        ];

        for (expected, actual) in expected_coordinates.iter().zip(iterator) {
            assert_eq!(expected, &actual);
        }
    }

    #[test]
    fn multiple_points_per_row() {
        let hover_text = vec!["text1".to_owned(), "text2".to_owned(), "text3".to_owned(), "text4".to_owned(), "text5".to_owned(), "text6".to_owned(), "text7".to_owned()];
        let rectangle_map = create_rectangle_map();
        let stages = vec![(1, "stageA".to_owned()),(1, "stageA".to_owned()),(1, "stageA".to_owned()),(2, "stageB".to_owned()),(2, "stageB".to_owned()),(2, "stageB".to_owned()),(3, "stageC".to_owned())];
        let mut rectangle_point_counts = HashMap::new();
        rectangle_point_counts.insert("stageA".to_owned(), 3);
        rectangle_point_counts.insert("stageB".to_owned(), 3);
        rectangle_point_counts.insert("stageC".to_owned(), 1);

        let iterator = MissedActionsCoordinatesIterator::new(&hover_text, &stages, &rectangle_map, &rectangle_point_counts, MAX_POINTS_PER_ROW);

        let expected_coordinates = vec![
            ("text1".to_owned(), seconds_to_date_time_string(33.0), -1.33),
            ("text2".to_owned(), seconds_to_date_time_string(66.0), -1.33),
            ("text3".to_owned(), seconds_to_date_time_string(50.0), -2.66),
            ("text4".to_owned(), seconds_to_date_time_string(133.0), -1.33),
            ("text5".to_owned(), seconds_to_date_time_string(166.0), -1.33),
            ("text6".to_owned(), seconds_to_date_time_string(150.0), -2.66),
            ("text7".to_owned(), seconds_to_date_time_string(250.0), -2.0)
        ];

        for (expected, actual) in expected_coordinates.iter().zip(iterator) {
            assert_eq!(expected, &actual);
        }
    }

    #[test]
    fn empty() {
        let hover_text: Vec<String> = vec![];
        let stages: Vec<(u32,String)> = vec![];
        let rectangle_map: HashMap<String, Rectangle> = HashMap::new();
        let rectangle_point_counts: HashMap<String, u16> = HashMap::new();

        let mut iterator = MissedActionsCoordinatesIterator::new(&hover_text, &stages, &rectangle_map, &rectangle_point_counts, MAX_POINTS_PER_ROW);

        assert!(iterator.next().is_none());
    }
}
#[cfg(test)]
mod tests_utils {
    mod calculate_points_per_row {
        use super::super::*;
        #[test]
        fn total_points_exact_multiple_of_max_points_per_row() {
            let total_points = 12;
            let max_points_per_row = 3;
            let expected_points_per_row = vec![3, 3, 3, 3];
            assert_eq!(calculate_points_per_row(total_points, max_points_per_row), expected_points_per_row);
        }
        #[test]
        fn total_points_more_than_max_points_per_row() {
            let total_points = 10;
            let max_points_per_row = 3;
            let expected_points_per_row = vec![3, 3, 3, 1];
            assert_eq!(calculate_points_per_row(total_points, max_points_per_row), expected_points_per_row);
        }

        #[test]
        fn total_points_less_than_max_points_per_row() {
            let total_points = 5;
            let max_points_per_row = 10;
            let expected_points_per_row = vec![5];
            assert_eq!(calculate_points_per_row(total_points, max_points_per_row), expected_points_per_row);
        }

        #[test]
        fn total_points_equal_to_max_points_per_row() {
            let total_points = 10;
            let max_points_per_row = 10;
            let expected_points_per_row = vec![10];
            assert_eq!(calculate_points_per_row(total_points, max_points_per_row), expected_points_per_row);
        }

        #[test]
        fn zero_total_points() {
            let total_points = 0;
            let max_points_per_row = 10;
            let expected_points_per_row: Vec<u16> = vec![];
            assert_eq!(calculate_points_per_row(total_points, max_points_per_row), expected_points_per_row);
        }

    }

    mod calculate_gaps_between_points_within_line {
        use super::super::*;

        #[test]
        fn single_point() {
            let line_length = 100.0;
            let num_gaps = 1;
            let expected_gap = 50.0;
            assert_eq!(calculate_gaps_between_points_within_line(line_length, num_gaps), expected_gap);
        }

        #[test]
        fn multiple_points() {
            let line_length = 100.0;
            let num_gaps = 5;
            let expected_gap = 16.67;
            assert_eq!(calculate_gaps_between_points_within_line(line_length, num_gaps), expected_gap);
        }

        #[test]
        fn zero_num_gaps() {
            let line_length = 100.0;
            let num_gaps = 0;
            assert_eq!(calculate_gaps_between_points_within_line(line_length, num_gaps), 100.0);
        }

        #[test]
        fn zero_line_length() {
            let line_length = 0.0;
            let num_gaps = 5;
            assert_eq!(calculate_gaps_between_points_within_line(line_length, num_gaps), 0.0);
        }

        #[test]
        fn negative_line_length() {
            let line_length = -100.0;
            let num_gaps = 5;
            let expected_gap = -16.66;
            assert_eq!(calculate_gaps_between_points_within_line(line_length, num_gaps), expected_gap);
        }
    }

    mod seconds_to_date_time_string {
        use super::super::*;
        use chrono::Utc;
        #[test]
        fn whole_seconds() {
            let seconds = 115.0;
            let today_start = Utc::now().with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
            let expected_date_time = today_start + chrono::Duration::seconds(seconds as i64);
            let expected_date_time_str = expected_date_time.format("%Y-%m-%d %H:%M:%S").to_string();
            assert_eq!(seconds_to_date_time_string(seconds), expected_date_time_str);
        }

        #[test]
        fn fractional_seconds() {
            let seconds = 115.5;
            let today_start = Utc::now().with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
            let expected_date_time = today_start + chrono::Duration::seconds(seconds as i64);
            let expected_date_time_str = expected_date_time.format("%Y-%m-%d %H:%M:%S").to_string();
            assert_eq!(seconds_to_date_time_string(seconds), expected_date_time_str);
        }

        #[test]
        fn negative_seconds() {
            let seconds = -115.0;
            let today_start = Utc::now().with_hour(0).unwrap().with_minute(0).unwrap().with_second(0).unwrap();
            let expected_date_time = today_start + chrono::Duration::seconds(seconds as i64);
            let expected_date_time_str = expected_date_time.format("%Y-%m-%d %H:%M:%S").to_string();
            assert_eq!(seconds_to_date_time_string(seconds), expected_date_time_str);
        }
    }

    mod calculate_point_coordinates {
        use super::super::*;

        #[test]
        fn single_point() {
            let rectangle = Rectangle { x0: 0.0, x1: 100.0, y0: 0.0, y1: 100.0, name: "stageA".to_owned() };
            let points_per_row = vec![1];
            let expected_coordinates = vec![(seconds_to_date_time_string(50.0), 50.0)];
            assert_eq!(calculate_point_coordinates(&rectangle, &points_per_row), expected_coordinates);
        }

        #[test]
        fn multiple_points_full_rows() {
            let rectangle = Rectangle { x0: 0.0, x1: 100.0, y0: -1.0, y1: -4.0, name: "stageA".to_owned() };
            let points_per_row = vec![2, 2];
            let expected_coordinates = vec![
                (seconds_to_date_time_string(33.0), -2.0),
                (seconds_to_date_time_string(66.0), -2.0),
                (seconds_to_date_time_string(33.0), -3.0),
                (seconds_to_date_time_string(66.0), -3.0),
            ];
            assert_eq!(calculate_point_coordinates(&rectangle, &points_per_row), expected_coordinates);
        }

        #[test]
        fn multiple_points_incomplete_row() {
            let rectangle = Rectangle { x0: 0.0, x1: 100.0, y0: -1.0, y1: -4.0, name: "stageA".to_owned() };
            let points_per_row = vec![2, 1];
            let expected_coordinates = vec![
                (seconds_to_date_time_string(33.0), -2.0),
                (seconds_to_date_time_string(66.0), -2.0),
                (seconds_to_date_time_string(50.0), -3.0),
            ];
            assert_eq!(calculate_point_coordinates(&rectangle, &points_per_row), expected_coordinates);
        }

        #[test]
        fn zero_points() {
            let rectangle = Rectangle { x0: 0.0, x1: 100.0, y0: 0.0, y1: 100.0, name: "stageA".to_owned() };
            let points_per_row = vec![0];
            assert!(calculate_point_coordinates(&rectangle, &points_per_row).is_empty());
        }
    }
}

