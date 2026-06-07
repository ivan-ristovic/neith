use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anyhow::Result;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as FuzzyConfig, Matcher, Utf32Str};
use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::Value;
use tantivy::{DocAddress, TantivyDocument};

use crate::indexer::{CatalogEntry, IndexHandle, IndexManager, SearchFields};
use crate::library::SourceKind;
use crate::man;
use crate::query::{
    LibraryScope, MatchMode, SearchRequest, SourceFilter, has_regex_meta, longest_literal_seed,
    normalize_query,
};

#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub path: PathBuf,
    pub rel_path: String,
    pub library_alias: String,
    pub source_kind: String,
    pub line: usize,
    pub snippet: String,
    pub score: f32,
    pub rank_reason: String,
    #[serde(skip)]
    pub body: String,
    #[serde(skip)]
    pub is_live_man: bool,
}

impl SearchResult {
    pub fn display_line(&self) -> String {
        let line = if self.line > 0 {
            format!(":{}", self.line)
        } else {
            String::new()
        };
        format!(
            "{:<7} {:<10} {}{}",
            self.source_kind, self.library_alias, self.rel_path, line
        )
    }
}

#[derive(Clone)]
pub struct SearchEngine {
    pub manager: IndexManager,
    pub man_cache_dir: PathBuf,
}

impl SearchEngine {
    pub fn new(manager: IndexManager) -> Self {
        let man_cache_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("neith")
            .join("man");
        Self {
            manager,
            man_cache_dir,
        }
    }

    pub fn reload(&self) -> Result<()> {
        self.manager.reload()
    }

    pub fn search(&self, request: &SearchRequest) -> Vec<SearchResult> {
        let query = normalize_query(&request.query);
        let mut results = Vec::new();
        if query.trim().is_empty() {
            return self.empty_query_results(request);
        }

        if matches!(request.filter, SourceFilter::All | SourceFilter::Man) {
            results.extend(man::lookup_live_man(&query, &self.man_cache_dir));
        }
        if matches!(
            request.filter,
            SourceFilter::All | SourceFilter::Names | SourceFilter::Man
        ) {
            results.extend(self.search_catalog_man_query(request, &query));
        }

        let indexed_results = self.search_tantivy(request, &query);
        results.extend(indexed_results);
        if request.mode == MatchMode::Fuzzy && results.is_empty() {
            results.extend(self.search_catalog_fuzzy(request, &query));
        }

        let mut by_path: HashMap<String, SearchResult> = HashMap::new();
        for result in results {
            let key = result.path.to_string_lossy().to_string();
            match by_path.get(&key) {
                Some(existing) if existing.score >= result.score => {}
                _ => {
                    by_path.insert(key, result);
                }
            }
        }
        let mut merged = by_path.into_values().collect::<Vec<_>>();
        merged.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.display_line().cmp(&right.display_line()))
        });
        merged.truncate(request.limit);
        merged
    }

    fn empty_query_results(&self, request: &SearchRequest) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for handle in self.matching_handles(&request.library) {
            for item in handle.catalog.iter().take(request.limit) {
                if source_matches(request.filter, &item.source_kind) {
                    results.push(result_from_catalog(item, 0.0, "catalog"));
                }
            }
        }
        results.truncate(request.limit);
        results
    }

    fn search_tantivy(&self, request: &SearchRequest, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let query_texts = query_variants_for_index(request.mode, query);
        if query_texts.is_empty() {
            return Vec::new();
        }
        let verification_regex = if request.mode == MatchMode::Exact && has_regex_meta(query) {
            match regex::RegexBuilder::new(query)
                .case_insensitive(true)
                .build()
            {
                Ok(regex) => Some(regex),
                Err(_) => return Vec::new(),
            }
        } else {
            None
        };

        for handle in self.matching_handles(&request.library) {
            let fields = fields_for_filter(&handle.fields, request.filter);
            let searcher = handle.reader.searcher();
            for query_text in &query_texts {
                let mut parser = QueryParser::for_index(&handle.index, fields.clone());
                parser.set_conjunction_by_default();
                let Ok(parsed) = parser.parse_query(query_text) else {
                    continue;
                };
                let limit = (request.limit * 6).max(30);
                let Ok(top_docs) =
                    searcher.search(&parsed, &TopDocs::with_limit(limit).order_by_score())
                else {
                    continue;
                };
                for (bm25, address) in top_docs {
                    if let Ok(mut result) = self.doc_to_result(handle, address, bm25, query) {
                        if !source_matches(request.filter, &result.source_kind) {
                            continue;
                        }
                        if let Some(regex) = &verification_regex {
                            let Some((line, snippet)) = regex_match_snippet(regex, &result) else {
                                continue;
                            };
                            result.line = line;
                            result.snippet = snippet;
                        }
                        apply_boosts(&mut result, query, request.filter, "index");
                        results.push(result);
                    }
                }
            }
        }
        results
    }

    fn search_catalog_man_query(&self, request: &SearchRequest, query: &str) -> Vec<SearchResult> {
        let Some(man_query) = man::split_man_query(query) else {
            return Vec::new();
        };
        let mut results = Vec::new();
        for handle in self.matching_handles(&request.library) {
            for item in &handle.catalog {
                if !source_matches(request.filter, &item.source_kind)
                    || item.source_kind != SourceKind::Man.as_str()
                {
                    continue;
                }
                let mut result = result_from_catalog(item, 0.0, "man-catalog");
                if man_page_boost(&result, &man_query) <= 0.0 {
                    continue;
                }
                apply_boosts(&mut result, query, request.filter, "man-catalog");
                results.push(result);
            }
        }
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(request.limit * 2);
        results
    }

    fn search_catalog_fuzzy(&self, request: &SearchRequest, query: &str) -> Vec<SearchResult> {
        let mut matcher = Matcher::new(FuzzyConfig::DEFAULT.match_paths());
        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );
        let mut buf = Vec::new();
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        let seed_terms = fuzzy_seed_terms(query);
        let mut candidates = Vec::new();

        for handle in self.matching_handles(&request.library) {
            for item in &handle.catalog {
                if !source_matches(request.filter, &item.source_kind) {
                    continue;
                }
                if !seen.insert(item.path.clone()) {
                    continue;
                }
                if seed_terms.is_empty() || catalog_matches_seed(item, &seed_terms) {
                    candidates.push(item);
                }
            }
        }

        if candidates.is_empty() && !seed_terms.is_empty() {
            seen.clear();
            for handle in self.matching_handles(&request.library) {
                for item in &handle.catalog {
                    if !source_matches(request.filter, &item.source_kind) {
                        continue;
                    }
                    if seen.insert(item.path.clone()) {
                        candidates.push(item);
                    }
                }
            }
        }

        for item in candidates {
            let combined = format!("{} {} {}", item.title, item.rel_path, item.excerpt);
            let Some(score) = pattern.score(Utf32Str::new(&combined, &mut buf), &mut matcher)
            else {
                continue;
            };
            let mut result = result_from_catalog(item, score as f32, "fuzzy");
            apply_boosts(&mut result, query, request.filter, "fuzzy");
            results.push(result);
        }
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(request.limit * 2);
        results
    }

    fn matching_handles<'a>(
        &'a self,
        scope: &'a LibraryScope,
    ) -> impl Iterator<Item = &'a IndexHandle> {
        self.manager
            .handles
            .iter()
            .filter(move |handle| match scope {
                LibraryScope::All => true,
                LibraryScope::Alias(alias) => &handle.library.alias == alias,
            })
    }

    fn doc_to_result(
        &self,
        handle: &IndexHandle,
        address: DocAddress,
        bm25: f32,
        query: &str,
    ) -> Result<SearchResult> {
        let searcher = handle.reader.searcher();
        let doc: TantivyDocument = searcher.doc(address)?;
        let fields = &handle.fields;
        let path = doc_text(&doc, fields.path_exact).unwrap_or_default();
        let rel_path = doc_text(&doc, fields.rel_path).unwrap_or_else(|| path.clone());
        let title = doc_text(&doc, fields.title).unwrap_or_else(|| rel_path.clone());
        let body = doc_text(&doc, fields.body).unwrap_or_default();
        let source_kind = doc_text(&doc, fields.source_kind).unwrap_or_else(|| "note".to_string());
        let library_alias =
            doc_text(&doc, fields.library).unwrap_or_else(|| handle.library.alias.clone());
        let (line, snippet) = best_snippet(&body, query);

        Ok(SearchResult {
            title,
            path: PathBuf::from(path),
            rel_path,
            library_alias,
            source_kind,
            line,
            snippet,
            score: bm25,
            rank_reason: "index".to_string(),
            body,
            is_live_man: false,
        })
    }
}

fn fields_for_filter(fields: &SearchFields, filter: SourceFilter) -> Vec<tantivy::schema::Field> {
    match filter {
        SourceFilter::Names | SourceFilter::Man => {
            vec![fields.title, fields.path_text, fields.rel_path]
        }
        SourceFilter::Content => vec![fields.body, fields.excerpt],
        SourceFilter::All => vec![
            fields.title,
            fields.path_text,
            fields.rel_path,
            fields.body,
            fields.excerpt,
        ],
    }
}

fn query_variants_for_index(mode: MatchMode, query: &str) -> Vec<String> {
    match mode {
        MatchMode::Exact if has_regex_meta(query) => longest_literal_seed(query)
            .map(|seed| vec![seed])
            .unwrap_or_default(),
        MatchMode::Exact => vec![query.to_string()],
        MatchMode::Fuzzy => fuzzy_index_variants(query),
    }
}

fn fuzzy_index_variants(query: &str) -> Vec<String> {
    let mut variants = vec![query.to_string()];

    if let Some(parts) = man::split_man_query(query) {
        let section = parts.section.unwrap_or_default();
        let mut section_variant = vec![parts.title.clone()];
        if !section.is_empty() {
            section_variant.push(section);
        }
        if !parts.remainder.is_empty() {
            section_variant.push(parts.remainder.clone());
        }
        variants.push(section_variant.join(" "));

        if !parts.remainder.is_empty() {
            variants.push(format!("{} {}", parts.title, parts.remainder));
        }
    }

    let terms = query.split_whitespace().collect::<Vec<_>>();
    for (idx, term) in terms.iter().enumerate() {
        for replacement in index_replacement_terms(term) {
            let mut replaced = terms.clone();
            replaced[idx] = replacement;
            variants.push(replaced.join(" "));
        }
    }

    dedupe_strings(variants)
}

fn index_replacement_terms(term: &str) -> Vec<&'static str> {
    match term {
        "column" => vec!["columns", "field", "fields"],
        "columns" => vec!["column", "field", "fields"],
        "field" => vec!["fields", "column", "columns"],
        "fields" => vec!["field", "column", "columns"],
        _ => Vec::new(),
    }
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty() && seen.insert(value.clone()))
        .collect()
}

fn fuzzy_seed_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(str::to_lowercase)
        .filter(|term| term.len() > 1)
        .collect()
}

fn catalog_matches_seed(item: &CatalogEntry, seed_terms: &[String]) -> bool {
    seed_terms.iter().any(|term| {
        text_contains_case_insensitive(&item.title, term)
            || text_contains_case_insensitive(&item.rel_path, term)
            || text_contains_case_insensitive(&item.excerpt, term)
    })
}

fn text_contains_case_insensitive(value: &str, needle: &str) -> bool {
    value.to_lowercase().contains(needle)
}

fn doc_text(doc: &TantivyDocument, field: tantivy::schema::Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn result_from_catalog(item: &CatalogEntry, score: f32, reason: &str) -> SearchResult {
    SearchResult {
        title: item.title.clone(),
        path: PathBuf::from(&item.path),
        rel_path: item.rel_path.clone(),
        library_alias: item.library_alias.clone(),
        source_kind: item.source_kind.clone(),
        line: 1,
        snippet: item.excerpt.clone(),
        score,
        rank_reason: reason.to_string(),
        body: String::new(),
        is_live_man: false,
    }
}

fn source_matches(filter: SourceFilter, source_kind: &str) -> bool {
    match filter {
        SourceFilter::All => true,
        SourceFilter::Names | SourceFilter::Content => {
            source_kind != SourceKind::Man.as_str() || filter == SourceFilter::Names
        }
        SourceFilter::Man => source_kind == SourceKind::Man.as_str(),
    }
}

fn apply_boosts(result: &mut SearchResult, query: &str, filter: SourceFilter, reason: &str) {
    let query_lower = query.to_lowercase();
    let title_lower = result.title.to_lowercase();
    let path_lower = result.rel_path.to_lowercase();
    let term_groups = query_term_groups(&query_lower);
    let man_query = man::split_man_query(&query_lower);
    let name_hits = term_groups
        .iter()
        .filter(|group| group_matches_name(group, &title_lower, &path_lower))
        .count() as f32;
    let all_name_hit = !term_groups.is_empty() && name_hits as usize == term_groups.len();
    let first_topic_hit = term_groups.first().is_some_and(|group| {
        path_lower
            .split('/')
            .next()
            .is_some_and(|segment| group.iter().any(|term| segment == term))
            || group.iter().any(|term| title_lower.contains(term.as_str()))
    });

    let source_boost = match result.source_kind.as_str() {
        "man" if result.is_live_man => 40_000.0,
        "man" if all_name_hit => 10_000.0,
        "note" if all_name_hit => 7_000.0,
        "devdocs" if all_name_hit => 5_000.0,
        "note" => 2_000.0,
        "devdocs" => 1_000.0,
        "man" => 3_000.0,
        _ => 0.0,
    };
    let topic_boost = match result.source_kind.as_str() {
        "note" if first_topic_hit => 5_000.0,
        "devdocs" if first_topic_hit => 2_000.0,
        "man" if first_topic_hit => 1_000.0,
        _ => 0.0,
    };
    let filter_boost = match filter {
        SourceFilter::Names if all_name_hit => 1_000.0,
        SourceFilter::Man if result.source_kind == "man" => 1_500.0,
        SourceFilter::Content => 200.0,
        _ => 0.0,
    };
    let man_boost = man_query
        .as_ref()
        .map(|query| man_page_boost(result, query))
        .unwrap_or(0.0);
    result.score += source_boost + topic_boost + filter_boost + man_boost + name_hits * 250.0;
    result.rank_reason = reason.to_string();
}

fn man_page_boost(result: &SearchResult, query: &man::ManQuery) -> f32 {
    if result.source_kind != SourceKind::Man.as_str() {
        return 0.0;
    }

    let page_slug = slug_man_name(&query.title);
    let rel_path = result.rel_path.to_lowercase();
    let title = result.title.to_lowercase();

    if let Some(section) = &query.section {
        let exact_title = format!("man: {} ({section})", query.title);
        let exact_live = format!("man:{}({section})", query.title);
        let exact_generated = format!("man/{page_slug}-{section}-");
        if title == exact_title || rel_path == exact_live || rel_path.starts_with(&exact_generated)
        {
            return 25_000.0;
        }
    }

    let generated_prefix = format!("man/{page_slug}-");
    let live_prefix = format!("man:{}(", query.title);
    let title_prefix = format!("man: {} ", query.title);
    if rel_path.starts_with(&generated_prefix)
        || rel_path.starts_with(&live_prefix)
        || title.starts_with(&title_prefix)
    {
        return 12_000.0;
    }

    0.0
}

fn slug_man_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn query_term_groups(query_lower: &str) -> Vec<Vec<String>> {
    query_lower
        .split_whitespace()
        .filter(|term| term.len() > 1)
        .map(term_aliases)
        .collect()
}

fn term_aliases(term: &str) -> Vec<String> {
    let mut aliases = vec![term.to_string()];
    match term {
        "column" | "columns" => {
            aliases.extend(["field", "fields"].into_iter().map(str::to_string));
        }
        "field" | "fields" => {
            aliases.extend(["column", "columns"].into_iter().map(str::to_string));
        }
        "selected" => {
            aliases.extend(["specific", "nth"].into_iter().map(str::to_string));
        }
        _ => {}
    }
    aliases
}

fn group_matches_name(group: &[String], title_lower: &str, path_lower: &str) -> bool {
    group
        .iter()
        .any(|term| title_lower.contains(term.as_str()) || path_lower.contains(term.as_str()))
}

fn best_snippet(body: &str, query: &str) -> (usize, String) {
    let terms = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut fallback = String::new();
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if fallback.is_empty() && !trimmed.is_empty() {
            fallback = trimmed.to_string();
        }
        let lower = trimmed.to_lowercase();
        if terms.iter().any(|term| lower.contains(term)) {
            return (idx + 1, trim_snippet(trimmed));
        }
    }
    (1, trim_snippet(&fallback))
}

fn trim_snippet(value: &str) -> String {
    const LIMIT: usize = 220;
    if value.len() <= LIMIT {
        value.to_string()
    } else {
        format!("{}...", truncate_to_char_boundary(value, LIMIT))
    }
}

fn truncate_to_char_boundary(value: &str, limit: usize) -> &str {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn regex_match_snippet(regex: &regex::Regex, result: &SearchResult) -> Option<(usize, String)> {
    if regex.is_match(&result.title) {
        return Some((1, trim_snippet(&result.title)));
    }
    if regex.is_match(&result.rel_path) {
        return Some((1, trim_snippet(&result.rel_path)));
    }
    for (idx, line) in result.body.lines().enumerate() {
        if regex.is_match(line) {
            return Some((idx + 1, trim_snippet(line.trim())));
        }
    }
    if let Some(found) = regex.find(&result.body) {
        let line = result.body[..found.start()]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let snippet = result
            .body
            .lines()
            .nth(line.saturating_sub(1))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(trim_snippet)
            .unwrap_or_else(|| trim_snippet(&result.body[found.start()..found.end()]));
        return Some((line, snippet));
    }
    if regex.is_match(&result.snippet) {
        return Some((result.line, trim_snippet(&result.snippet)));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_prefers_matching_line() {
        let (line, snippet) = best_snippet("alpha\nbeta gamma\n", "gamma");
        assert_eq!(line, 2);
        assert_eq!(snippet, "beta gamma");
    }

    #[test]
    fn source_filter_matches_man() {
        assert!(source_matches(SourceFilter::Man, "man"));
        assert!(!source_matches(SourceFilter::Man, "note"));
    }

    #[test]
    fn ordinal_column_query_boosts_selected_field_titles() {
        let mut selected = SearchResult {
            title: "Print Selected Fields With awk".to_string(),
            path: PathBuf::from("/tmp/awk/print-selected-fields.md"),
            rel_path: "awk/print-selected-fields.md".to_string(),
            library_alias: "test".to_string(),
            source_kind: "note".to_string(),
            line: 1,
            snippet: String::new(),
            score: 100.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: false,
        };
        let mut aligned = SearchResult {
            title: "Print Aligned Table With awk".to_string(),
            path: PathBuf::from("/tmp/awk/print-aligned-table.md"),
            rel_path: "awk/print-aligned-table.md".to_string(),
            library_alias: "test".to_string(),
            source_kind: "note".to_string(),
            line: 1,
            snippet: String::new(),
            score: 120.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: false,
        };

        apply_boosts(
            &mut selected,
            "awk print selected column",
            SourceFilter::All,
            "fuzzy",
        );
        apply_boosts(
            &mut aligned,
            "awk print selected column",
            SourceFilter::All,
            "fuzzy",
        );

        assert!(selected.score > aligned.score);
    }

    #[test]
    fn sectioned_man_query_boosts_exact_page() {
        let mut exact = SearchResult {
            title: "man: printf (3)".to_string(),
            path: PathBuf::from("/tmp/man/printf-3.md"),
            rel_path: "man/printf-3-a88efd3cb696.md".to_string(),
            library_alias: "devdocs".to_string(),
            source_kind: "man".to_string(),
            line: 1,
            snippet: String::new(),
            score: 100.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: false,
        };
        let mut distractor = SearchResult {
            title: "man: gnutls_psk_format_imported_identity (3)".to_string(),
            path: PathBuf::from("/tmp/man/gnutls.md"),
            rel_path: "man/gnutls-psk-format-imported-identity-3.md".to_string(),
            library_alias: "devdocs".to_string(),
            source_kind: "man".to_string(),
            line: 1,
            snippet: String::new(),
            score: 5_000.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: false,
        };

        apply_boosts(&mut exact, "printf(3) format", SourceFilter::All, "fuzzy");
        apply_boosts(
            &mut distractor,
            "printf(3) format",
            SourceFilter::All,
            "fuzzy",
        );

        assert!(exact.score > distractor.score);
    }

    #[test]
    fn live_man_boosts_above_indexed_devdocs_man() {
        let mut live = SearchResult {
            title: "man: printf(3)".to_string(),
            path: PathBuf::from("/tmp/live/printf.3.txt"),
            rel_path: "man:printf(3)".to_string(),
            library_alias: "live-man".to_string(),
            source_kind: "man".to_string(),
            line: 1,
            snippet: String::new(),
            score: 100.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: true,
        };
        let mut indexed = SearchResult {
            title: "man: printf (3)".to_string(),
            path: PathBuf::from("/tmp/devdocs/man/printf-3.md"),
            rel_path: "man/printf-3-a88efd3cb696.md".to_string(),
            library_alias: "devdocs".to_string(),
            source_kind: "man".to_string(),
            line: 1,
            snippet: String::new(),
            score: 5_000.0,
            rank_reason: String::new(),
            body: String::new(),
            is_live_man: false,
        };

        apply_boosts(&mut live, "printf(3) format", SourceFilter::All, "live-man");
        apply_boosts(&mut indexed, "printf(3) format", SourceFilter::All, "index");

        assert!(live.score > indexed.score);
    }

    #[test]
    fn trims_snippet_on_utf8_boundary() {
        let snippet = trim_snippet(&"é".repeat(200));
        assert!(snippet.ends_with("..."));
        assert!(snippet.is_char_boundary(snippet.len()));
    }

    #[test]
    fn regex_matches_full_body() {
        let result = SearchResult {
            title: "title".to_string(),
            path: PathBuf::from("/tmp/doc.md"),
            rel_path: "doc.md".to_string(),
            library_alias: "test".to_string(),
            source_kind: "note".to_string(),
            line: 1,
            snippet: "front matter".to_string(),
            score: 1.0,
            rank_reason: String::new(),
            body: "intro\nalpha then beta\n".to_string(),
            is_live_man: false,
        };
        let regex = regex::RegexBuilder::new("alpha.*beta")
            .case_insensitive(true)
            .build()
            .unwrap();

        assert_eq!(
            regex_match_snippet(&regex, &result),
            Some((2, "alpha then beta".to_string()))
        );
    }

    #[test]
    fn regex_matches_across_body_lines() {
        let result = SearchResult {
            title: "title".to_string(),
            path: PathBuf::from("/tmp/doc.md"),
            rel_path: "doc.md".to_string(),
            library_alias: "test".to_string(),
            source_kind: "note".to_string(),
            line: 1,
            snippet: String::new(),
            score: 1.0,
            rank_reason: String::new(),
            body: "intro\nalpha\nbeta\n".to_string(),
            is_live_man: false,
        };
        let regex = regex::RegexBuilder::new("(?s)alpha.*beta")
            .case_insensitive(true)
            .build()
            .unwrap();

        assert_eq!(
            regex_match_snippet(&regex, &result),
            Some((2, "alpha".to_string()))
        );
    }
}
