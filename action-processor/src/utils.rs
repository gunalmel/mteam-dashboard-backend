use std::string::ToString;
pub(crate) fn normalize_whitespace(input: &str) -> String {
    input
        .trim()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
pub(crate) fn capitalize_words(input: &str) -> String {
    input
        .trim()
        .split_whitespace()
        .map(|word| {
            if word.chars().all(|c| c.is_numeric() || c.is_uppercase()) {
                return word.to_string();
            }

            if word.starts_with('(') {
                return format!("({}", capitalize_words(&word[1..word.len()])); // Recurse to handle nested parentheses
            }

            let mut chars = word.chars();
            let first_char = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            let rest: String = chars.as_str().to_lowercase();
            first_char + &rest
        })
        .collect::<Vec<String>>()
        .join(" ")
        .replace(" ( ", " (")
        .replace(") ", ") ")
        .replace(" )", ")")
}
#[cfg(test)]
mod tests {
    mod test_normalize_whitespace {
        use super::super::*;

        #[test]
        fn whitespace_basic() {
            let input = "   Hello   World   ";
            let expected = "Hello World";
            assert_eq!(normalize_whitespace(input), expected);
        }

        #[test]
        fn whitespace_multiple_spaces() {
            let input = "Rust    is     awesome!";
            let expected = "Rust is awesome!";
            assert_eq!(normalize_whitespace(input), expected);
        }

        #[test]
        fn whitespace_empty_string() {
            let input = "";
            let expected = "";
            assert_eq!(normalize_whitespace(input), expected);
        }

        #[test]
        fn whitespace_only_spaces() {
            let input = "      ";
            let expected = "";
            assert_eq!(normalize_whitespace(input), expected);
        }

        #[test]
        fn with_tabs_and_newlines() {
            let input = "Hello\t\tWorld\n\nRust    ";
            let expected = "Hello World Rust";
            assert_eq!(normalize_whitespace(input), expected);
        }
    }

    mod test_capitalize_words {
        use super::super::*;
        #[test]
        fn capitalize() {
            assert_eq!(capitalize_words("hello world"), "Hello World");
            assert_eq!(capitalize_words("   rust is awesome   "), "Rust Is Awesome");
            assert_eq!(capitalize_words("multiple   spaces   here"), "Multiple Spaces Here");
            assert_eq!(capitalize_words(""), "");
            assert_eq!(capitalize_words("   "), "");
            assert_eq!(capitalize_words("already Capitalized"), "Already Capitalized");
            assert_eq!(capitalize_words("single"), "Single");
            assert_eq!(capitalize_words("123 testing numbers"), "123 Testing Numbers");
            assert_eq!(capitalize_words("Defib (UNsynchronized Shock) 200J"), "Defib (Unsynchronized Shock) 200J");
            assert_eq!(capitalize_words(" Defib   ( UNsynchronized   Shock  )   100J "), "Defib (Unsynchronized Shock) 100J");
            assert_eq!(capitalize_words("(parentheses) around words"), "(Parentheses) Around Words");
            assert_eq!(capitalize_words("punctuation, should work!"), "Punctuation, Should Work!");
            assert_eq!(capitalize_words("100j test"), "100j Test");
            assert_eq!(capitalize_words("Order EKG"), "Order EKG");
            assert_eq!(capitalize_words("  Order  EKG    test  "), "Order EKG Test");
        }
    }
}