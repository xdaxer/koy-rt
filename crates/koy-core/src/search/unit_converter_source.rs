use super::{
    ISearchShell, ISearchSource, SearchDragData, SearchDragKind, SearchGlyph, SearchQuery, SearchRelevance, SearchResult,
    SearchSourceBase, UnitConverter,
};
use crate::clock::Culture;
use std::sync::Arc;

use super::Translate;

/// Birim çevirici: "10 km to mi", "100 c in f", "1 gb in mb". Ağ yok; Enter yalnız sayıyı (kültüre göre, binliksiz)
/// panoya kopyalar, satırda birimli hâli görünür.
pub struct UnitConverterSource {
    pub base: SearchSourceBase,
    shell: Arc<dyn ISearchShell>,
    t: Translate,
    culture: Culture,
}

impl UnitConverterSource {
    /// C# `culture = null` → `CurrentCulture`: burada zorunlu parametre.
    pub fn new(shell: Arc<dyn ISearchShell>, t: Translate, culture: Culture) -> Self {
        let name = t("SearchCatUnits");
        UnitConverterSource { base: SearchSourceBase::new("units", name), shell, t, culture }
    }
}

impl ISearchSource for UnitConverterSource {
    crate::search_source_base_methods!(base);

    fn is_busy(&self) -> bool {
        false
    }

    fn prepare(&self) {}

    fn release(&self) {}

    fn search(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let Some(conversion) = UnitConverter::try_convert(Some(&query.raw), &self.culture) else {
            return Vec::new();
        };

        let number = UnitConverter::format_number(conversion.value, &self.culture, false);
        if number.is_empty() {
            return Vec::new();
        }

        let input = UnitConverter::format_number(conversion.input, &self.culture, true);
        let title = format!("{} {}", UnitConverter::format_number(conversion.value, &self.culture, true), conversion.to.symbol);
        let shell = Arc::clone(&self.shell);
        let copy = number.clone();
        let mut result = SearchResult::new(format!("unit:{}", query.raw), self.base.name(), title, move || shell.copy_text(&copy));
        result.subtitle = Some(format!("{} {} · {}", input, conversion.from.symbol, (self.t)("SearchHintCopy")));
        result.glyph = SearchGlyph::Calculator;
        result.category_relevance = SearchRelevance::HIGHEST;
        result.relevance = 1.0;
        result.restores_focus = true;
        result.drag = Some(SearchDragData::new(SearchDragKind::Text, number));
        vec![result]
    }
}
