use chrono::{Datelike, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use crate::utils;
use crate::plot_structures::CsvRowTime;

lazy_static! {
    static ref ACTION_NAME_REGEX: Regex = Regex::new(r"^\s*\((\d+)\)\s*(.+?)\s*\(action\)\s*$").unwrap();
    static ref SHOCK_VALUE_REGEX: Regex = Regex::new(r"(.*?)(\b\d+[Jj]\b)(.*)").unwrap(); 
}
pub fn parse_time(input: &str) -> Option<CsvRowTime> {
    // Split the input into hours, minutes, and seconds
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 3 {
        return None; // Input format is invalid
    }

    // Parse hours, minutes, and seconds
    let hours: u32 = parts[0].parse().ok()?;
    let minutes: u32 = parts[1].parse().ok()?;
    let seconds: u32 = parts[2].parse().ok()?;

    // Validate the ranges
    if minutes >= 60 || seconds >= 60 {
        return None; // Invalid time input
    }

    // Calculate total seconds
    let total_seconds = hours * 3600 + minutes * 60 + seconds;

    // Get today's UTC date
    let today = Utc::now();
    let date_string = format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}",
        today.year(),
        today.month(),
        today.day(),
        hours,
        minutes,
        seconds
    );

    // Format the input into HH:MM:SS
    let formatted_input = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

    Some(CsvRowTime {
        total_seconds,
        date_string,
        timestamp: formatted_input,
    })
}

pub fn extract_stage_name(input: &str) -> Option<(u32, String)> {
    ACTION_NAME_REGEX.captures(input).and_then(|captures| {
        let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
        let action_name = captures.get(2).map(|matched| {
            utils::normalize_whitespace(matched.as_str())
        })?;
        Some((number, action_name))
    })
}

pub fn extract_shock_value(input: &str) -> (String, String) {
    match SHOCK_VALUE_REGEX.captures(input).map(|captures| {
        let before = captures.get(1).map_or("", |m| m.as_str()).trim();
        let value = captures.get(2).map_or("", |m| m.as_str()).trim();
        let after = captures.get(3).map_or("", |m| m.as_str()).trim();

        (format!("{} {}", before, after).trim().to_owned(), value.to_owned())
    }){
        Some((action_name, joule)) => {
            if joule.is_empty() {
                (action_name, "".to_owned())
            } else {
                (action_name, joule)
            }
        },
        None => (input.to_owned(), "".to_owned())
    }
}

pub fn process_action_name(input: &str) -> (String, String, String) {
    let (normalized_action_name, joule) = extract_shock_value(utils::capitalize_words(input).replace("UNAVAILABLE", "").trim());
    let corrected_action_name = match normalized_action_name.as_str() {
        "Ascultate Lungs" => "Auscultate Lungs",
        "SYNCHRONIZED Shock" => "Synchronized Shock",
        _ => normalized_action_name.as_str(),
    };

    let category = match corrected_action_name {
        "Select Amiodarone" => "Medication",
        "Select Calcium" => "Medication",
        "Select Epinephrine" => "Medication",
        "Select Lidocaine" => "Medication",
        _ => corrected_action_name,
    };

    let action_name = if joule.is_empty() { corrected_action_name.to_owned() } else { format!("{} {}", corrected_action_name, joule) };
    (action_name, category.to_owned(), joule)
}

#[cfg(test)]
mod tests {
    mod test_parse_time {
        use super::super::*;
        use chrono::DateTime;

        #[test]
        fn valid_time() {
            let timestamp = "12:34:56";
            let parsed_time = parse_time(timestamp).unwrap();
            let now: DateTime<Utc> = Utc::now();
            let formatted_date_string = now.format("%Y-%m-%d").to_string() + " " + timestamp;

            assert_eq!(parsed_time.timestamp, timestamp);
            assert_eq!(parsed_time.total_seconds, 12 * 3600 + 34 * 60 + 56);
            assert_eq!(parsed_time.date_string,formatted_date_string);
        }

        #[test]
        fn invalid_time() {
            let timestamp = "invalid time";
            let result = parse_time(timestamp);
            assert!(result.is_none());
        }

        #[test]
        fn empty_time() {
            let timestamp = "";
            let result = parse_time(timestamp);
            assert!(result.is_none());
        }
    }

    mod test_extract_stage_name {
        use super::super::*;

        #[test]
        fn valid() {
            let input = "(123)example(action)";
            let expected = Some((123, "example".to_owned()));
            assert_eq!(extract_stage_name(input), expected);
        }

        #[test]
        fn no_action() {
            let input = "(123)example";
            let expected = None;
            assert_eq!(extract_stage_name(input), expected);
        }

        #[test]
        fn empty() {
            let input = "";
            let expected = None;
            assert_eq!(extract_stage_name(input), expected);
        }

        #[test]
        fn no_match() {
            let input = "example(action)";
            let expected = None;
            assert_eq!(extract_stage_name(input), expected);
        }

        #[test]
        fn with_spaces() {
            let input = "  (123)   example   with spaces   (action)   ";
            let expected = Some((123,"example with spaces".to_owned()));
            assert_eq!(extract_stage_name(input), expected);
        }
    }

    mod test_etract_shock_value {
        use super::super::*;

        #[test]
        fn basic() {
            assert_eq!(extract_shock_value("xyz rts 100J klm abc"), ("xyz rts klm abc".to_owned(), "100J".to_owned()));
        }

        #[test]
        fn lowercase_j() {
            assert_eq!(extract_shock_value("xyz rts 100j klm abc"), ("xyz rts klm abc".to_owned(), "100j".to_owned()));
        }

        #[test]
        fn no_value() {
            assert_eq!(extract_shock_value("no value here"), ("no value here".to_owned(), "".to_owned()));
        }

        #[test]
        fn at_beginning() {
            assert_eq!(extract_shock_value("123J at the beginning"), ("at the beginning".to_owned(), "123J".to_owned()));
        }

        #[test]
        fn at_end() {
            assert_eq!(extract_shock_value("at the end 456j"), ("at the end".to_owned(), "456j".to_owned()));
        }

        #[test]
        fn multiple_values() {
            assert_eq!(extract_shock_value("multiple 789J values 123j in string"), ("multiple values 123j in string".to_owned(), "789J".to_owned()));
        }

        #[test]
        fn leading_trailing_spaces() {
            assert_eq!(extract_shock_value(" leading and trailing spaces 100J "), ("leading and trailing spaces".to_owned(), "100J".to_owned()));
        }

        #[test]
        fn only_value() {
            assert_eq!(extract_shock_value("100J"), ("".to_owned(), "100J".to_owned()));
        }

        #[test]
        fn with_spaces_around() {
            assert_eq!(extract_shock_value("test   100J   test"), ("test test".to_owned(), "100J".to_owned()));
        }

        #[test]
        fn no_letters_around_value() {
            assert_eq!(extract_shock_value("100Jtest"), ("100Jtest".to_owned(), "".to_owned()));
        }

        #[test]
        fn at_the_very_end() {
            assert_eq!(extract_shock_value("test 100J"), ("test".to_owned(), "100J".to_owned()));
        }

        #[test]
        fn at_the_very_beginning() {
            assert_eq!(extract_shock_value("100J test"), ("test".to_owned(), "100J".to_owned()));
        }
    }

    mod test_process_action_name {
        use super::super::*;

        #[test]
        fn process_action_names() {
            let test_cases = [
                ("Ascultate Lungs", ("Auscultate Lungs".to_owned(), "Auscultate Lungs".to_owned(), "".to_owned())),
                ("Auscultate Lungs", ("Auscultate Lungs".to_owned(), "Auscultate Lungs".to_owned(), "".to_owned())),
                ("Check Lab Tests", ("Check Lab Tests".to_owned(), "Check Lab Tests".to_owned(), "".to_owned())),
                ("Defib (UNsynchronized Shock) 100J", ("Defib (Unsynchronized Shock) 100J".to_owned(), "Defib (Unsynchronized Shock)".to_owned(), "100J".to_owned())),
                ("Defib (UNsynchronized Shock) 200J", ("Defib (Unsynchronized Shock) 200J".to_owned(), "Defib (Unsynchronized Shock)".to_owned(), "200J".to_owned())),
                ("Defib (UNsynchronized Shock) 300J", ("Defib (Unsynchronized Shock) 300J".to_owned(), "Defib (Unsynchronized Shock)".to_owned(), "300J".to_owned())),
                ("Insert Bag Mask", ("Insert Bag Mask".to_owned(), "Insert Bag Mask".to_owned(), "".to_owned())),
                ("Insert Lactated Ringers (1 Liter)", ("Insert Lactated Ringers (1 Liter)".to_owned(), "Insert Lactated Ringers (1 Liter)".to_owned(), "".to_owned())),
                ("Insert Syringe on Right Hand", ("Insert Syringe On Right Hand".to_owned(), "Insert Syringe On Right Hand".to_owned(), "".to_owned())),
                ("Measure Glucose Level", ("Measure Glucose Level".to_owned(), "Measure Glucose Level".to_owned(), "".to_owned())),
                ("Order Chest X-ray", ("Order Chest X-ray".to_owned(), "Order Chest X-ray".to_owned(), "".to_owned())),
                ("Order Cooling", ("Order Cooling".to_owned(), "Order Cooling".to_owned(), "".to_owned())),
                ("Order EKG", ("Order EKG".to_owned(), "Order EKG".to_owned(), "".to_owned())),
                ("Order Intubation", ("Order Intubation".to_owned(), "Order Intubation".to_owned(), "".to_owned())),
                ("Order Needle Thoracostomy", ("Order Needle Thoracostomy".to_owned(), "Order Needle Thoracostomy".to_owned(), "".to_owned())),
                ("Order new Labs UNAVAILABLE", ("Order New Labs".to_owned(), "Order New Labs".to_owned(), "".to_owned())),
                ("Order Pericardiocentesis", ("Order Pericardiocentesis".to_owned(), "Order Pericardiocentesis".to_owned(), "".to_owned())),
                ("Order Ultrasound", ("Order Ultrasound".to_owned(), "Order Ultrasound".to_owned(), "".to_owned())),
                ("Perform Bag Mask Pump", ("Perform Bag Mask Pump".to_owned(), "Perform Bag Mask Pump".to_owned(), "".to_owned())),
                ("Pulse Check", ("Pulse Check".to_owned(), "Pulse Check".to_owned(), "".to_owned())),
                ("Select Amiodarone", ("Select Amiodarone".to_owned(), "Medication".to_owned(), "".to_owned())),
                ("Select Calcium", ("Select Calcium".to_owned(), "Medication".to_owned(), "".to_owned())),
                ("Select Epinephrine", ("Select Epinephrine".to_owned(), "Medication".to_owned(), "".to_owned())),
                ("Select Lidocaine", ("Select Lidocaine".to_owned(), "Medication".to_owned(), "".to_owned())),
                ("SYNCHRONIZED Shock 100J", ("Synchronized Shock 100J".to_owned(), "Synchronized Shock".to_owned(), "100J".to_owned())),
                ("SYNCHRONIZED Shock 200J", ("Synchronized Shock 200J".to_owned(), "Synchronized Shock".to_owned(), "200J".to_owned())),
                ("View Cardiac Arrest Guidelines", ("View Cardiac Arrest Guidelines".to_owned(), "View Cardiac Arrest Guidelines".to_owned(), "".to_owned())),
            ];

            for (input, expected) in test_cases {
                let result = process_action_name(input);
                assert_eq!(result, expected);
            }
        }
    }
}