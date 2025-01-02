use std::collections::HashSet;
use tracing::debug;

#[derive(Debug, PartialOrd, PartialEq, Clone, Default)]
pub struct Tags {
    tag: String,
    pub tags: Vec<String>,
}

impl Tags {
    pub fn new(tag: Option<String>) -> Self {
        Tags {
            tag: Tags::create_normalized_tag_string(tag.clone()),
            tags: Tags::normalize_tag_string(tag),
        }
    }
    /// creates list of normalized tags from "tag1,t2,t3" string
    /// be aware of shell parsing rules, so no blanks or quotes
    pub fn normalize_tag_string(tag_str: Option<String>) -> Vec<String> {
        match tag_str {
            Some(s) => {
                let _tags = s
                    .replace(' ', "")
                    .split(',')
                    .map(|s| s.trim().to_lowercase().to_owned())
                    .collect::<Vec<_>>();
                Self::clean_tags(_tags)
            }
            None => Vec::new(),
        }
    }

    pub fn clean_tags(tags: Vec<String>) -> Vec<String> {
        let mut _tags: Vec<String> = tags
            .iter()
            .flat_map(|s| s.split(','))
            .map(|s| s.trim().to_lowercase().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        _tags.sort();
        _tags.dedup();
        debug!("{:?}", _tags);
        _tags
    }

    pub fn create_normalized_tag_string(tag_str: Option<String>) -> String {
        format!(",{},", Self::normalize_tag_string(tag_str).join(","))
    }

    pub fn change_tag_string_delimiter(tag_str: &str, new_delimiter: &str) -> String {
        /*
         * tag_str is a normalized string with the following format :
         * ",tag1,tag2,tag3,"
         * cf. create_normalized_tag_string function in tag.rs
         *
         * We turn this normalized tags string
         * into a string ready for stdout :
         * so that, with delimiter e.g " | ",
         * we get :
         * "tag1 | tag2 | tag3"
         */
        let mut tags = tag_str.split(',').collect::<Vec<_>>();
        tags.retain(|&x| !x.is_empty());
        tags.join(new_delimiter)
    }

    pub fn match_exact_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
        let set1: HashSet<String> = tags.iter().map(|s| s.to_string()).collect();
        let set2: HashSet<String> = bm_tags.iter().map(|s| s.to_string()).collect();
        set1 == set2
    }

    pub fn match_all_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
        let set1: HashSet<_> = tags.iter().collect();
        let set2: HashSet<_> = bm_tags.iter().collect();
        let intersect = set1.intersection(&set2).collect::<HashSet<_>>();
        intersect == set1.iter().collect()
    }

    pub fn match_any_tags(tags: &Vec<String>, bm_tags: &Vec<String>) -> bool {
        let set1: HashSet<_> = tags.iter().collect();
        let set2: HashSet<_> = bm_tags.iter().collect();
        let intersect = set1.intersection(&set2).collect::<HashSet<_>>();
        !intersect.is_empty()
    }
}

#[cfg(test)]
mod test {
    use crate::model::tag::Tags;
    use rstest::*;
    use tracing::debug;

    #[rstest]
    fn test_default() {
        let tags = Tags::default();
        assert_eq!(tags.tags.len(), 0);
        debug!("{:?}", tags);
    }

    #[rstest]
    #[case(Some("a,b".to_string()), ",a,b,".to_string(), vec ! ["a".to_string(), "b".to_string()])]
    #[case(Some(",,,b,a".to_string()), ",a,b,".to_string(), vec ! ["a".to_string(), "b".to_string()])]
    #[case(None, ",,".to_string(), vec ! [])]
    fn test_tags(
        #[case] tag: Option<String>,
        #[case] expected: String,
        #[case] expected_vec: Vec<String>,
    ) {
        let tags = Tags::new(tag.clone());
        assert_eq!(tags.tag, expected);
        assert_eq!(tags.tags, expected_vec);
        debug!("{:?}", tags);
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
        assert_eq!(Tags::clean_tags(input), expected)
    }

    #[rstest]
    #[case(Some("tag1,tag2".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("tag2,tag1".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("tag2,,tag1".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some(",tag2,,tag1,".to_string()), String::from(",tag1,tag2,"))]
    #[case(Some("".to_string()), String::from(",,"))]
    fn test_create_normalized_tag_string(#[case] input: Option<String>, #[case] expected: String) {
        assert_eq!(Tags::create_normalized_tag_string(input), expected)
    }

    #[rstest]
    #[case(& vec ! ["a", "b"], & vec ! ["a", "b"], true)]
    #[case(& vec ! [], & vec ! [], true)]
    #[case(& vec ! ["a", "b"], & vec ! ["a",], false)]
    fn test_match_exact_tags(
        #[case] tags: &Vec<&str>,
        #[case] bm_tags: &Vec<&str>,
        #[case] expected: bool,
    ) {
        let tags = &tags.iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(Tags::match_exact_tags(tags, bm_tags), expected)
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
        let tags = &tags.iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(Tags::match_all_tags(tags, bm_tags), expected)
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
        let tags = &tags.iter().map(|s| s.to_string()).collect();
        let bm_tags = &bm_tags.iter().map(|s| s.to_string()).collect();
        debug!("{:?}, {:?} {:?}", tags, bm_tags, expected);
        assert_eq!(Tags::match_any_tags(tags, bm_tags), expected)
    }
}
