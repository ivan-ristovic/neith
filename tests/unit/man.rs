use super::*;

#[test]
fn parses_name_section_query() {
    assert_eq!(
        split_man_query("printf(3) format"),
        Some(ManQuery {
            title: "printf".to_string(),
            section: Some("3".to_string()),
            remainder: "format".to_string()
        })
    );
}

#[test]
fn parses_section_name_query() {
    assert_eq!(
        split_man_query("3 printf format").unwrap().section,
        Some("3".to_string())
    );
}
