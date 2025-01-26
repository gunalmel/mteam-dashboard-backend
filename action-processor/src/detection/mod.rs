use std::borrow::ToOwned;
use crate::action_csv_row::ActionCsvRow;
use crate::utils;
use crate::plot_structures::{CsvRowTime, PlotLocation};

const CPR_START_MARKERS: [&'static str; 2] = ["begin cpr", "enter cpr"];
const CPR_END_MARKERS: [&'static str; 2]  = ["stop cpr", "end cpr"];
const ERROR_MARKER_TIME_THRESHOLD: u32 = 2;

pub fn is_action_row(csv_row: &ActionCsvRow) -> bool {
    csv_row.parsed_stage.is_some() &&
        !csv_row.subaction_time.trim().is_empty() &&
        !csv_row.subaction_name.is_empty() &&
        !is_missed_action(csv_row) &&
        csv_row.cpr_boundary.is_none()
}

pub fn is_stage_boundary(csv_row: &ActionCsvRow) -> bool {
    csv_row.parsed_stage.is_some() &&
        csv_row.subaction_time.trim().is_empty() &&
        csv_row.subaction_name.is_empty() &&
        csv_row.score.is_empty() &&
        csv_row.old_value.is_empty() &&
        csv_row.new_value.is_empty()
}

pub fn cpr_boundary(csv_row: &ActionCsvRow) -> Option<String> {
    let normalized_action_name = utils::normalize_whitespace(csv_row.subaction_name.to_lowercase().as_str());
    if CPR_START_MARKERS.contains(&&*normalized_action_name) { Some("START".to_owned()) } else if
        CPR_END_MARKERS.contains(&&*normalized_action_name) { Some("END".to_owned()) } else { None }
}
pub fn is_error_action_marker(csv_row: &ActionCsvRow) -> bool {
    csv_row.old_value.trim() == "Error-Triggered" &&
        csv_row.score.trim() == "Action-Was-Performed"
}

pub fn is_missed_action(csv_row: &ActionCsvRow) -> bool {
    csv_row.old_value.trim() == "Error-Triggered" &&
        csv_row.score.trim() == "Action-Was-Not-Performed"
}

pub fn check_cpr(csv_row: &ActionCsvRow) -> Option<(String, PlotLocation)> {
    csv_row.cpr_boundary.clone().and_then(|cpr_boundary| {
        Some((cpr_boundary, PlotLocation::new(csv_row)))
    })
}

pub fn can_mark_each_other(csv_row1: &ActionCsvRow, csv_row2: &ActionCsvRow) -> bool{
    let marker_time: u32 = csv_row1.timestamp.clone().unwrap_or(CsvRowTime::default()).total_seconds;
    let current_time: u32 = csv_row2.timestamp.clone().unwrap_or(CsvRowTime::default()).total_seconds;

    marker_time.abs_diff(current_time)<=ERROR_MARKER_TIME_THRESHOLD
}

pub fn is_erroneous_action(csv_row: &ActionCsvRow, error_marker_row: &ActionCsvRow) -> bool{
    csv_row.action_point && error_marker_row.username == csv_row.action_vital_name &&
        can_mark_each_other(csv_row, error_marker_row)
}

#[cfg(test)]
mod tests {
    mod test_is_action_row {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::is_action_row;

        #[test]
        fn is_true() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "12:34".to_owned(),
                subaction_name: "SubAction".to_owned(),
                ..Default::default()
            };
            assert!(is_action_row(&csv_row));
        }

        #[test]
        fn is_false_no_parsed_action_name() {
            let csv_row = ActionCsvRow {
                parsed_stage: None,
                subaction_time: "12:34".to_owned(),
                subaction_name: "SubAction".to_owned(),
                ..Default::default()
            };
            assert!(!is_action_row(&csv_row));
        }

        #[test]
        fn is_false_empty_subaction_time() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "SubAction".to_owned(),
                ..Default::default()
            };
            assert!(!is_action_row(&csv_row));
        }

        #[test]
        fn is_false_empty_subaction_name() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "12:34".to_owned(),
                subaction_name: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_action_row(&csv_row));
        }

        #[test]
        fn is_false_missed_action() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "12:34".to_owned(),
                subaction_name: "SubAction".to_owned(),
                old_value: "Error-Triggered".to_owned(),
                score: "Action-Was-Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_action_row(&csv_row));
        }

        #[test]
        fn is_false_cpr_boundary() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "12:34".to_owned(),
                subaction_name: "SubAction".to_owned(),
                cpr_boundary: Some("START".to_owned()),
                ..Default::default()
            };
            assert!(!is_action_row(&csv_row));
        }
    }

    mod test_is_stage_boundary {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::is_stage_boundary;

        #[test]
        fn is_true() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "".to_owned(),
                score: "".to_owned(),
                old_value: "".to_owned(),
                new_value: "".to_owned(),
                ..Default::default()
            };
            assert!(is_stage_boundary(&csv_row));
        }

        #[test]
        fn is_false_non_empty_subaction_time() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "12:34".to_owned(),
                subaction_name: "".to_owned(),
                score: "".to_owned(),
                old_value: "".to_owned(),
                new_value: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_stage_boundary(&csv_row));
        }

        #[test]
        fn is_false_non_empty_subaction_name() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "SubAction".to_owned(),
                score: "".to_owned(),
                old_value: "".to_owned(),
                new_value: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_stage_boundary(&csv_row));
        }

        #[test]
        fn is_false_non_empty_score() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "".to_owned(),
                score: "Score".to_owned(),
                old_value: "".to_owned(),
                new_value: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_stage_boundary(&csv_row));
        }

        #[test]
        fn is_false_non_empty_old_value() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "".to_owned(),
                score: "".to_owned(),
                old_value: "OldValue".to_owned(),
                new_value: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_stage_boundary(&csv_row));
        }

        #[test]
        fn is_false_non_empty_new_value() {
            let csv_row = ActionCsvRow {
                parsed_stage: Some((1,"Action".to_owned())),
                subaction_time: "".to_owned(),
                subaction_name: "".to_owned(),
                score: "".to_owned(),
                old_value: "".to_owned(),
                new_value: "NewValue".to_owned(),
                ..Default::default()
            };
            assert!(!is_stage_boundary(&csv_row));
        }
    }

    mod test_cpr_boundary {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::cpr_boundary;

        #[test]
        fn is_start() {
            let csv_row = ActionCsvRow {
                subaction_name: "begin cpr".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), Some("START".to_owned()));
        }

        #[test]
        fn is_end() {
            let csv_row = ActionCsvRow {
                subaction_name: "stop cpr".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), Some("END".to_owned()));
        }

        #[test]
        fn is_none() {
            let csv_row = ActionCsvRow {
                subaction_name: "other action".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), None);
        }

        #[test]
        fn is_start_case_insensitive() {
            let csv_row = ActionCsvRow {
                subaction_name: "Begin CPR".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), Some("START".to_owned()));
        }

        #[test]
        fn is_end_case_insensitive() {
            let csv_row = ActionCsvRow {
                subaction_name: "Stop CPR".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), Some("END".to_owned()));
        }

        #[test]
        fn is_none_with_whitespace() {
            let csv_row = ActionCsvRow {
                subaction_name: "  begin cpr  ".to_owned(),
                ..Default::default()
            };
            assert_eq!(cpr_boundary(&csv_row), Some("START".to_owned()));
        }
    }

    mod test_is_error_action_marker {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::is_error_action_marker;

        #[test]
        fn is_true() {
            let csv_row = ActionCsvRow {
                old_value: "Error-Triggered".to_owned(),
                score: "Action-Was-Performed".to_owned(),
                ..Default::default()
            };
            assert!(is_error_action_marker(&csv_row));
        }

        #[test]
        fn is_false_wrong_old_value() {
            let csv_row = ActionCsvRow {
                old_value: "Not-Error".to_owned(),
                score: "Action-Was-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_error_action_marker(&csv_row));
        }

        #[test]
        fn is_false_wrong_score() {
            let csv_row = ActionCsvRow {
                old_value: "Error-Triggered".to_owned(),
                score: "Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_error_action_marker(&csv_row));
        }

        #[test]
        fn is_false_both_wrong() {
            let csv_row = ActionCsvRow {
                old_value: "Not-Error".to_owned(),
                score: "Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_error_action_marker(&csv_row));
        }

        #[test]
        fn is_false_empty_values() {
            let csv_row = ActionCsvRow {
                old_value: "".to_owned(),
                score: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_error_action_marker(&csv_row));
        }
    }

    mod test_is_missed_action {
        use super::super::*;
        use crate::detection::is_missed_action;

        #[test]
        fn is_true() {
            let csv_row = ActionCsvRow {
                old_value: "Error-Triggered".to_owned(),
                score: "Action-Was-Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(is_missed_action(&csv_row));
        }

        #[test]
        fn is_false_wrong_old_value() {
            let csv_row = ActionCsvRow {
                old_value: "Not-Error".to_owned(),
                score: "Action-Was-Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_missed_action(&csv_row));
        }

        #[test]
        fn is_false_wrong_score() {
            let csv_row = ActionCsvRow {
                old_value: "Error-Triggered".to_owned(),
                score: "Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_missed_action(&csv_row));
        }

        #[test]
        fn is_false_both_wrong() {
            let csv_row = ActionCsvRow {
                old_value: "Not-Error".to_owned(),
                score: "Not-Performed".to_owned(),
                ..Default::default()
            };
            assert!(!is_missed_action(&csv_row));
        }

        #[test]
        fn is_false_empty_values() {
            let csv_row = ActionCsvRow {
                old_value: "".to_owned(),
                score: "".to_owned(),
                ..Default::default()
            };
            assert!(!is_missed_action(&csv_row));
        }
    }

    mod test_check_cpr{
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::check_cpr;
        use crate::plot_structures::{CsvRowTime, PlotLocation};

        #[test]
        fn is_none() {
            let expected_plot_location = PlotLocation {
                timestamp: CsvRowTime {
                    total_seconds: 3600,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                },
                stage: (1,"Stage 1".to_owned())
            };

            let csv_row = ActionCsvRow {
                timestamp: Some(expected_plot_location.timestamp),
                parsed_stage: Some(expected_plot_location.stage),
                ..Default::default()
            };

            assert_eq!(None, check_cpr(&csv_row));
        }
        #[test]
        fn is_beginning() {
            let expected_plot_location = PlotLocation {
                timestamp: CsvRowTime {
                    total_seconds: 3600,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                },
                stage: (1,"Stage 1".to_owned())
            };
            let expected = Some((String::from("START"), expected_plot_location.clone()));

            let csv_row = ActionCsvRow {
                timestamp: Some(expected_plot_location.timestamp),
                parsed_stage: Some(expected_plot_location.stage),
                cpr_boundary: Some("START".to_owned()),
                ..Default::default()
            };

            assert_eq!(expected, check_cpr(&csv_row));
        }

        #[test]
        fn is_end() {
            let expected_plot_location = PlotLocation {
                timestamp: CsvRowTime {
                    total_seconds: 3600,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                },
                stage: (1,"Stage 1".to_owned())
            };
            let expected = Some((String::from("END"), expected_plot_location.clone()));

            let csv_row = ActionCsvRow {
                timestamp: Some(expected_plot_location.timestamp),
                parsed_stage: Some(expected_plot_location.stage),
                cpr_boundary: Some("END".to_owned()),
                ..Default::default()
            };

            assert_eq!(expected, check_cpr(&csv_row));
        }
    }

    mod test_can_mark_each_other {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::{can_mark_each_other, ERROR_MARKER_TIME_THRESHOLD};
        use crate::plot_structures::CsvRowTime;

        #[test]
        fn within_threshold() {
            let time = 3600;
            let csv_row1 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time + ERROR_MARKER_TIME_THRESHOLD,
                    date_string: "2024-12-24 01:00:02".to_owned(),
                    timestamp: "01:00:02".to_owned(),
                }),
                ..Default::default()
            };
            assert!(can_mark_each_other(&csv_row1, &csv_row2));
        }

        #[test]
        fn within_threshold_opposite_direction() {
            let time = 3600;
            let csv_row1 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: 3600,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time - ERROR_MARKER_TIME_THRESHOLD,
                    date_string: "2024-12-24 01:00:02".to_owned(),
                    timestamp: "01:00:02".to_owned(),
                }),
                ..Default::default()
            };
            assert!(can_mark_each_other(&csv_row1, &csv_row2));
        }

        #[test]
        fn exceeds_threshold() {
            let time = 3600;
            let csv_row1 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time + ERROR_MARKER_TIME_THRESHOLD + 1,
                    date_string: "2024-12-24 01:00:03".to_owned(),
                    timestamp: "01:00:03".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!can_mark_each_other(&csv_row1, &csv_row2));
        }

        #[test]
        fn exceeds_threshold_opposite_direction() {
            let time = 3600;
            let csv_row1 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: time - ERROR_MARKER_TIME_THRESHOLD - 1,
                    date_string: "2024-12-24 01:00:03".to_owned(),
                    timestamp: "01:00:03".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!can_mark_each_other(&csv_row1, &csv_row2));
        }

        #[test]
        fn no_timestamp() {
            let csv_row1 = ActionCsvRow {
                timestamp: None,
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: Some(CsvRowTime {
                    total_seconds: 3600,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!can_mark_each_other(&csv_row1, &csv_row2));
        }

        #[test]
        fn both_no_timestamp() {
            let csv_row1 = ActionCsvRow {
                timestamp: None,
                ..Default::default()
            };
            let csv_row2 = ActionCsvRow {
                timestamp: None,
                ..Default::default()
            };
            assert!(can_mark_each_other(&csv_row1, &csv_row2));
        }
    }

    mod test_is_erroneous_action {
        use crate::action_csv_row::ActionCsvRow;
        use crate::detection::{is_erroneous_action, ERROR_MARKER_TIME_THRESHOLD};
        use crate::plot_structures::CsvRowTime;

        fn create_csv_row(time: u32) -> (u32, ActionCsvRow) {

            let csv_row = ActionCsvRow {
                action_point: true,
                action_vital_name: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            (time, csv_row)
        }

        #[test]
        fn is_true() {
            let time = 3600;
            let (time, csv_row) = create_csv_row(time);
            let error_marker_row = ActionCsvRow {
                username: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time-ERROR_MARKER_TIME_THRESHOLD,
                    date_string: "2024-12-24 01:00:02".to_owned(),
                    timestamp: "01:00:02".to_owned(),
                }),
                ..Default::default()
            };

            assert!(is_erroneous_action(&csv_row, &error_marker_row));
        }

        #[test]
        fn is_false_different_stages() {
            let time = 3600;
            let csv_row = ActionCsvRow {
                action_point: true,
                action_vital_name: "X".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let error_marker_row = ActionCsvRow {
                username: "(1)Stage A(action)".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time+ERROR_MARKER_TIME_THRESHOLD,
                    date_string: "2024-12-24 01:00:02".to_owned(),
                    timestamp: "01:00:02".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!is_erroneous_action(&csv_row, &error_marker_row));
        }

        #[test]
        fn is_false_time_threshold_exceeded() {
            let time = 3600;
            let (time, csv_row) = create_csv_row(time);
            let error_marker_row = ActionCsvRow {
                username: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time+ERROR_MARKER_TIME_THRESHOLD+1,
                    date_string: "2024-12-24 01:00:05".to_owned(),
                    timestamp: "01:00:05".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!is_erroneous_action(&csv_row, &error_marker_row));
        }

        #[test]
        fn is_false_time_threshold_exceeded_opposite_direction() {
            let time = 3600;
            let (time, csv_row) = create_csv_row(time);
            let error_marker_row = ActionCsvRow {
                username: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time-ERROR_MARKER_TIME_THRESHOLD-1,
                    date_string: "2024-12-24 01:00:05".to_owned(),
                    timestamp: "01:00:05".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!is_erroneous_action(&csv_row, &error_marker_row));
        }

        #[test]
        fn is_false_not_action_point() {
            let time = 3600;
            let csv_row = ActionCsvRow {
                action_point: false,
                action_vital_name: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time,
                    date_string: "2024-12-24 01:00:00".to_owned(),
                    timestamp: "01:00:00".to_owned(),
                }),
                ..Default::default()
            };
            let error_marker_row = ActionCsvRow {
                username: "User1".to_owned(),
                timestamp: Some(CsvRowTime {
                    total_seconds: time+ERROR_MARKER_TIME_THRESHOLD-1,
                    date_string: "2024-12-24 01:00:02".to_owned(),
                    timestamp: "01:00:02".to_owned(),
                }),
                ..Default::default()
            };
            assert!(!is_erroneous_action(&csv_row, &error_marker_row));
        }
    }
}