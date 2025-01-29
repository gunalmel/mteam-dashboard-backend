/*
 * uses flat_map instead of map for the to_uppercase() and to_lowercase() calls.
 * This avoids the need for the match statement to handle empty words, as flat_map will simply produce
 * no output if the iterator is empty.
 */
pub fn snake_case_file_to_title_case(file_name: &str) -> String {
    let snake_case_string = file_name.split('.').next().unwrap_or(file_name);
    snake_case_string.split('_')
        .map(|word| {
            word.chars().take(1).flat_map(|c| c.to_uppercase())
                .chain(word.chars().skip(1).flat_map(|c| c.to_lowercase()))
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snake_case_file_to_title_case() {
        assert_eq!(snake_case_file_to_title_case("example_file_name.txt"), "Example File Name");
        assert_eq!(snake_case_file_to_title_case("another_example_file.rs"), "Another Example File");
        assert_eq!(snake_case_file_to_title_case("singleword"), "Singleword");
        assert_eq!(snake_case_file_to_title_case(""), "");
        assert_eq!(snake_case_file_to_title_case("file_with_multiple.parts.txt"), "File With Multiple");
        assert_eq!(snake_case_file_to_title_case("file_with_MiXeD.CaSE.txt"), "File With Mixed");
        assert_eq!(snake_case_file_to_title_case("file_without_Extension"), "File Without Extension");
    }
}