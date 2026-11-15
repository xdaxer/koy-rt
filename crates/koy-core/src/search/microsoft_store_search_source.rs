use super::language_search_sources::SearchPrefix;
use super::search_util::{escape_data_string, format_template};
use super::{
    ISearchShell, ISearchSource, SearchAction, SearchDragData, SearchDragKind, SearchGlyph, SearchQuery, SearchRelevance,
    SearchResult, SearchSourceBase, Translate,
};
use std::sync::Arc;

/// Microsoft Store çevrimiçi araması: "store <terim>" veya "mağaza <terim>".
pub struct MicrosoftStoreSearchSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
}

impl MicrosoftStoreSearchSource {
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate) -> Self {
        let name = t("SearchCatStore");
        MicrosoftStoreSearchSource { base: SearchSourceBase::new("store", name), shell, t }
    }
}

impl ISearchSource for MicrosoftStoreSearchSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(term) = SearchPrefix::try_extract(&query.raw, &(self.t)("SearchKwStore")) else {
            return Vec::new();
        };

        let escaped = escape_data_string(&term);
        let store_uri = format!("ms-windows-store://search/?query={escaped}");
        let web_url = format!("https://apps.microsoft.com/search?q={escaped}");

        let shell = Arc::clone(&self.shell);
        let mut result = SearchResult::new(
            format!("store:{term}"),
            self.base.name(),
            format_template(&(self.t)("SearchStoreFor"), &[&term]),
            move || shell.open(&store_uri),
        );
        result.subtitle = Some("Microsoft Store".to_string());
        result.glyph = SearchGlyph::Store;
        result.category_relevance = SearchRelevance::HIGH;
        result.relevance = 0.9;
        result.drag = Some(SearchDragData::new(SearchDragKind::Url, web_url.clone()));
        let shell = Arc::clone(&self.shell);
        result.actions = vec![SearchAction::new("web", (self.t)("SearchActOpenWeb"), move || shell.open(&web_url))];
        vec![result]
    }
}
