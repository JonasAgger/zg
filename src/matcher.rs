use regex::Regex;

pub(crate) trait MatchEngine: Clone + Send {
    fn match_line(&self, line: &str) -> bool;
}

#[derive(Clone)]
pub(crate) struct ContainsMatcher {
    pattern: String,
}

#[derive(Clone)]
pub(crate) struct RegexMatcher {
    matcher: Regex,
}

impl ContainsMatcher {
    pub fn new(pattern: &String) -> Self {
        Self {
            pattern: pattern.clone(),
        }
    }
}

impl RegexMatcher {
    pub fn new(pattern: &String) -> Self {
        Self {
            matcher: Regex::new(pattern.as_str()).unwrap(),
        }
    }
}

impl MatchEngine for ContainsMatcher {
    fn match_line(&self, line: &str) -> bool {
        line.contains(self.pattern.as_str())
    }
}

impl MatchEngine for RegexMatcher {
    fn match_line(&self, line: &str) -> bool {
        self.matcher.is_match(line)
    }
}
