#![crate_type = "lib"]
#![crate_name = "bkmr"]
// #![allow(unused_variables, unused_imports)]

extern crate skim;

use std::collections::HashSet;

#[allow(unused_imports)]
use stdext::function_name;

pub mod bms;
pub mod dal;
pub mod environment;
pub mod fzf;
pub mod helper;
pub mod models;
pub mod process;
pub mod schema;

pub fn clean_tags(tags: Vec<String>) -> Vec<String> {
    // let tags = HashSet::new();
    // let _tags = tags.iter().map(|x| x.split(',').collect()).collect();
    let mut _tags: Vec<String> = tags
        .iter()
        .flat_map(|s| s.split(','))
        .map(|s| s.trim().to_lowercase().to_owned())
        .filter(|s| s.ne(""))
        .collect();
    _tags.sort();
    _tags.dedup();
    println!("{:?}", _tags);
    _tags
}

fn match_exact_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
    let set1: HashSet<String> = tags.into_iter().map(|s| s.to_string()).collect();
    let set2: HashSet<String> = bm_tags.into_iter().map(|s| s.to_string()).collect();
    set1 == set2
}

fn match_all_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
    let set1: HashSet<String> = tags.into_iter().map(|s| s.to_string()).collect();
    let set2: HashSet<String> = bm_tags.into_iter().map(|s| s.to_string()).collect();
    let intersect = set1.intersection(&set2).collect::<HashSet<_>>();
    intersect == set1.iter().collect()
}

fn match_any_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
    let set1: HashSet<String> = tags.into_iter().map(|s| s.to_string()).collect();
    let set2: HashSet<String> = bm_tags.into_iter().map(|s| s.to_string()).collect();
    let intersect = set1.intersection(&set2).collect::<HashSet<_>>();
    intersect.len() > 0
}

/// creates list of normalized tags from "tag1,t2,t3" string
/// be aware of shell parsing rules, so no blanks or quotes
fn normalize_tag_string(tag_str: Option<String>) -> Vec<String> {
    match tag_str {
        Some(s) => {
            let mut _tags = s
                .replace(" ", "")
                .split(",")
                .map(|s| s.trim().to_lowercase().to_owned())
                .collect::<Vec<_>>();
            clean_tags(_tags)
        }
        None => Vec::new(),
    }
}

pub fn create_normalized_tag_string(tag_str: Option<String>) -> String {
    format!(",{},", normalize_tag_string(tag_str).join(","))
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}

#[cfg(test)]
mod test {
    use log::debug;
    use rstest::*;

    use crate::{
        clean_tags, create_normalized_tag_string, match_all_tags, match_any_tags, match_exact_tags,
        normalize_tag_string,
    };

    fn parse_tags(tags: Vec<String>) -> String {
        let _tags = clean_tags(tags);
        format!(",{},", _tags.join(","))
    }

    #[rstest]
    #[case(vec ! ["a,A", ",b", "A"], vec ! ["a", "b"])]
    #[case(vec ! [], vec ! [])]
    #[case(vec ! ["b", "a"], vec ! ["a", "b"])]
    #[case(vec ! ["a", ",b"], vec ! ["a", "b"])]
    #[case(vec ! ["a,", ",b"], vec ! ["a", "b"])]
    #[case(vec ! ["a,xxx", ",b"], vec ! ["a", "b", "xxx"])]
    #[case(vec ! ["a,", ",b", "A"], vec ! ["a", "b"])]
    fn test_clean_tags(#[case] input: Vec<&str>, #[case] expected: Vec<&str>) {
        // let input = vec!("a", "b");
        // let expected = vec!("a", "b");
        let input = input.iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?}", input, expected);
        assert_eq!(clean_tags(input), expected)
    }

    #[rstest]
    #[case(vec ! ["tag1", "tag2"], String::from(",tag1,tag2,"))]
    #[case(vec ! ["tag2", "tag1"], ",tag1,tag2,")]
    #[case(vec ! [], ",,")]
    fn test_parse_tags(#[case] input: Vec<&str>, #[case] expected: String) {
        let input = input.iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?}", input, expected);
        assert_eq!(parse_tags(input), expected)
    }

    /*
    #[rstest]
    #[case(vec ! ["a", "b"], vec ! ["a", "b"])]
    #[case(vec ! ["xxx", "yyy"], vec ! [])]
    #[case(vec ! ["xxx", "yyy", "zzz"], vec ! ["zzz"],)]
    #[case(vec ! [], vec ! [])]
    fn test_check_tags(#[case] input: Vec<&str>, #[case] expected: Vec<&str>) {
        debug!("{:?}, {:?}", input, expected);
    }
    */

    #[rstest]
    #[case(& vec ! ["a", "b"], & vec ! ["a", "b"], true)]
    #[case(& vec ! [], & vec ! [], true)]
    #[case(& vec ! ["a", "b"], & vec ! ["a",], false)]
    fn test_match_exact_tags(
        #[case] tags: &Vec<&str>,
        #[case] bm_tags: &Vec<&str>,
        #[case] expected: bool,
    ) {
        let tags = &tags.into_iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.into_iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(match_exact_tags(tags, bm_tags), expected)
    }

    #[rstest]
    #[case(& vec ! ["a", "b"], & vec ! ["a", "b", "c", "d"], true)]
    #[case(& vec ! ["a", "b"], & vec ! ["b", "c", "d"], false)]
    #[case(& vec ! ["a", "b"], & vec ! ["b", "a"], true)]
    #[case(& vec ! ["a", "b"], & vec ! ["a",], false)]
    fn test_match_all_tags(
        #[case] tags: &Vec<&str>,
        #[case] bm_tags: &Vec<&str>,
        #[case] expected: bool,
    ) {
        let tags = &tags.into_iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.into_iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(match_all_tags(tags, bm_tags), expected)
    }

    #[rstest]
    #[case(& vec ! ["a", "b"], & vec ! ["a", "b", "c", "d"], true)]
    #[case(& vec ! ["a", "b", "x"], & vec ! ["a", "b", "c", "d"], true)]
    #[case(& vec ! ["a", "b", "x"], & vec ! ["a",], true)]
    #[case(& vec ! ["a", "b"], & vec ! ["x", "y"], false)]
    fn test_match_any_tags(
        #[case] tags: &Vec<&str>,
        #[case] bm_tags: &Vec<&str>,
        #[case] expected: bool,
    ) {
        let tags = &tags.into_iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.into_iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(match_any_tags(tags, bm_tags), expected)
    }

    #[rstest]
    #[case(Some("tag1,tag2".to_string()), vec ! ["tag1", "tag2"])]
    #[case(Some("tag1, tag2".to_string()), vec ! ["tag1", "tag2"])]
    #[case(Some("tag2,tag1".to_string()), vec ! ["tag1", "tag2"])]
    #[case(Some(" tag2,tag1 ".to_string()), vec ! ["tag1", "tag2"])]
    #[case(None, vec ! [])]
    fn test_normalize_tag_string(#[case] input: Option<String>, #[case] expected: Vec<&str>) {
        let expected: Vec<String> = expected.iter().map(|x| x.to_string()).collect();
        debug!("{:?}, {:?}", input, expected);
        assert_eq!(normalize_tag_string(input), expected)
    }

    #[rstest]
    #[case(Some("tag1,tag2".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("tag2,tag1".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("tag2,,tag1".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some(",tag2,,tag1,".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("".to_string()), String::from(",,"))]
    fn test_create_normalized_tag_string(#[case] input: Option<String>, #[case] expected: String) {
        assert_eq!(create_normalized_tag_string(input), expected)
    }
}
