//! Karşılığı: `tests/Core.Tests/JumpList/CustomDestinationsReaderTests.cs`.

use super::custom_destinations_builder::{link, link_spec, link_strings, CustomDestinationsBuilder, LinkSpec};
use crate::jump_list::{CustomDestinationsCategoryKind, CustomDestinationsReader, JumpListKnownCategory, JumpListLimits};

fn parse(data: &[u8]) -> crate::jump_list::CustomDestinationsFile {
    CustomDestinationsReader::parse(data)
}

#[test]
fn reads_tasks_custom_and_known_categories_in_file_order() {
    let data = CustomDestinationsBuilder::new()
        .tasks(&[link(), link_strings(&["a"])])
        .custom("Son klasörler", &[link(), link(), link()])
        .known(2)
        .build();

    let file = parse(&data);

    assert_eq!(file.categories.len(), 3);
    let c = &file.categories[0];
    assert_eq!(c.kind, CustomDestinationsCategoryKind::Tasks);
    assert_eq!(c.items.len(), 2);
    let c = &file.categories[1];
    assert_eq!(c.kind, CustomDestinationsCategoryKind::Custom);
    assert_eq!(c.name.as_deref(), Some("Son klasörler"));
    assert_eq!(c.items.len(), 3);
    let c = &file.categories[2];
    assert_eq!(c.kind, CustomDestinationsCategoryKind::Known);
    assert_eq!(c.known, Some(JumpListKnownCategory::Recent));
    assert!(c.items.is_empty());
    assert!(file.declares(JumpListKnownCategory::Recent));
    assert!(!file.declares(JumpListKnownCategory::Frequent));
}

#[test]
fn item_bytes_are_the_link_stream_without_the_clsid() {
    let l = link_strings(&["--yeni"]);
    let file = parse(&CustomDestinationsBuilder::new().tasks(&[l.clone()]).build());

    assert_eq!(l, file.categories[0].items[0]);
}

#[test]
fn frequent_known_category_is_recognised() {
    let file = parse(&CustomDestinationsBuilder::new().known(1).build());

    assert!(file.declares(JumpListKnownCategory::Frequent));
}

#[test]
fn unknown_known_category_id_is_skipped_but_file_stays_valid() {
    let file = parse(&CustomDestinationsBuilder::new().known(-1).tasks(&[link()]).build());

    assert_eq!(file.categories.len(), 1);
    assert_eq!(file.categories[0].kind, CustomDestinationsCategoryKind::Tasks);
}

#[test]
fn measures_links_with_idlist_linkinfo_ansi_strings_and_extra_blocks() {
    let data = CustomDestinationsBuilder::new()
        .tasks(&[
            link_spec(LinkSpec {
                strings: Some(vec!["gorev"]),
                string_flags: 0x04 | 0x20,
                unicode: false,
                id_list_bytes: 10,
                link_info_bytes: 20,
                extra_blocks: 2,
            }),
            link_spec(LinkSpec {
                strings: Some(vec!["ad", "yol", "klasor", "arg", "simge"]),
                string_flags: 0x04 | 0x08 | 0x10 | 0x20 | 0x40,
                id_list_bytes: 0,
                link_info_bytes: 0,
                extra_blocks: 1,
                ..Default::default()
            }),
            link_spec(LinkSpec { strings: Some(vec![]), string_flags: 0, ..Default::default() }),
        ])
        .build();

    let file = parse(&data);

    assert_eq!(file.categories.len(), 1);
    assert_eq!(file.categories[0].items.len(), 3);
}

#[test]
fn custom_category_with_empty_name_is_read() {
    let file = parse(&CustomDestinationsBuilder::new().custom("", &[link()]).build());

    assert_eq!(file.categories.len(), 1);
    assert_eq!(file.categories[0].name.as_deref(), Some(""));
}

// --- Bozuk / kısaltılmış / bilinmeyen biçim: panik yok, boş sonuç ---

#[test]
fn empty_and_tiny_inputs_are_empty() {
    assert!(parse(&[]).is_empty());
    assert!(parse(&[0u8; 5]).is_empty());
    assert!(parse(&[0u8; 12]).is_empty());
}

#[test]
fn unknown_version_is_empty() {
    let data = CustomDestinationsBuilder::new().tasks(&[link()]).build_with(3, None);

    assert!(parse(&data).is_empty());
}

#[test]
fn bad_category_count_is_empty() {
    for count in [0, -1, JumpListLimits::MAX_CATEGORIES + 1] {
        let data = CustomDestinationsBuilder::new().tasks(&[link()]).build_with(2, Some(count));

        assert!(parse(&data).is_empty(), "sayı {count}");
    }
}

#[test]
fn every_truncation_of_a_valid_file_is_empty_and_never_panics() {
    let data = CustomDestinationsBuilder::new()
        .tasks(&[
            link(),
            link_spec(LinkSpec {
                strings: Some(vec!["x", "y"]),
                string_flags: 0x04 | 0x20,
                id_list_bytes: 6,
                link_info_bytes: 8,
                extra_blocks: 1,
                ..Default::default()
            }),
        ])
        .custom("Kategori", &[link()])
        .known(2)
        .build();

    assert!(!parse(&data).is_empty()); // sağlam hâli okunur

    for length in 0..data.len() {
        let truncated = data[..length].to_vec();
        assert!(parse(&truncated).is_empty(), "uzunluk {length}");
    }
}

#[test]
fn missing_footer_is_empty() {
    let mut data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    let last = data.len() - 1;
    data[last] ^= 0xFF;

    assert!(parse(&data).is_empty());
}

#[test]
fn item_that_is_not_a_shell_link_is_empty() {
    let mut data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    data[12 + 4 + 4] ^= 0xFF; // ilk öğenin CLSID'inin ilk baytı

    assert!(parse(&data).is_empty());
}

#[test]
fn unknown_category_type_is_empty() {
    let mut data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    data[12] = 7;

    assert!(parse(&data).is_empty());
}

#[test]
fn link_header_with_wrong_size_is_empty() {
    let mut l = link();
    l[0] = 0x10;

    assert!(parse(&CustomDestinationsBuilder::new().tasks(&[l]).build()).is_empty());
}

#[test]
fn extra_block_size_pointing_past_the_end_is_empty() {
    let mut l = link_spec(LinkSpec { extra_blocks: 1, ..Default::default() });
    // ek bloğun boyutunu dosyadan büyük yap
    let block_offset = l.len() - 4 - 12;
    l[block_offset..block_offset + 4].copy_from_slice(&0x7FFFFFF0u32.to_le_bytes());

    assert!(parse(&CustomDestinationsBuilder::new().tasks(&[l]).build()).is_empty());
}

#[test]
fn link_info_size_below_four_is_empty() {
    let mut l = link_spec(LinkSpec { link_info_bytes: 0, ..Default::default() });
    let info_offset = 0x4C;
    l[info_offset..info_offset + 4].copy_from_slice(&2u32.to_le_bytes());

    assert!(parse(&CustomDestinationsBuilder::new().tasks(&[l]).build()).is_empty());
}

#[test]
fn negative_item_count_is_empty() {
    let mut data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    data[12 + 4..12 + 8].copy_from_slice(&(-5i32).to_le_bytes());

    assert!(parse(&data).is_empty());
}

#[test]
fn item_count_over_the_limit_is_empty() {
    let items: Vec<Vec<u8>> = (0..JumpListLimits::MAX_ITEMS_PER_CATEGORY + 1).map(|_| link()).collect();

    assert!(parse(&CustomDestinationsBuilder::new().tasks(&items).build()).is_empty());
}

#[test]
fn item_count_at_the_limit_is_read() {
    let items: Vec<Vec<u8>> = (0..JumpListLimits::MAX_ITEMS_PER_CATEGORY).map(|_| link()).collect();

    let file = parse(&CustomDestinationsBuilder::new().tasks(&items).build());

    assert_eq!(file.categories.len(), 1);
    assert_eq!(file.categories[0].items.len(), JumpListLimits::MAX_ITEMS_PER_CATEGORY as usize);
}

#[test]
fn huge_declared_item_count_does_not_allocate_or_panic() {
    let mut data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    data[12 + 4..12 + 8].copy_from_slice(&i32::MAX.to_le_bytes());

    assert!(parse(&data).is_empty());
}

#[test]
fn total_item_count_over_the_limit_is_empty() {
    let mut builder = CustomDestinationsBuilder::new();
    // 5 kategori x 64 öğe = 320 > 256
    for c in 0..5 {
        let items: Vec<Vec<u8>> = (0..JumpListLimits::MAX_ITEMS_PER_CATEGORY).map(|_| link()).collect();
        builder.add_custom(&format!("k{c}"), &items);
    }

    assert!(parse(&builder.build()).is_empty());
}

#[test]
fn category_name_over_the_limit_is_empty() {
    let name = "a".repeat(JumpListLimits::MAX_CATEGORY_NAME_CHARS as usize + 1);

    assert!(parse(&CustomDestinationsBuilder::new().custom(&name, &[link()]).build()).is_empty());
}

#[test]
fn negative_name_length_is_empty() {
    let mut data = CustomDestinationsBuilder::new().custom("ab", &[link()]).build();
    data[12 + 4..12 + 6].copy_from_slice(&(-3i16).to_le_bytes());

    assert!(parse(&data).is_empty());
}

#[test]
fn file_over_the_size_limit_is_empty() {
    let data = CustomDestinationsBuilder::new().tasks(&[link()]).build();
    let mut big = vec![0u8; JumpListLimits::MAX_FILE_BYTES as usize + 1];
    big[..data.len()].copy_from_slice(&data);

    assert!(parse(&big).is_empty());
}

#[test]
fn single_item_over_the_size_limit_is_empty() {
    let l = link_spec(LinkSpec { id_list_bytes: 60000, link_info_bytes: 10000, ..Default::default() });

    assert!(parse(&CustomDestinationsBuilder::new().tasks(&[l]).build()).is_empty());
}

/// Basit, tohumlu üreteç (C#'taki `new Random(12345)` yerine; testin tek beklentisi sınırlardır).
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn random_garbage_never_panics() {
    let mut rng = Xorshift(12345);
    let valid = CustomDestinationsBuilder::new().tasks(&[link(), link()]).custom("k", &[link()]).known(2).build();

    for _round in 0..3000 {
        let mut data = valid.clone();
        let flips = 1 + rng.below(5);
        for _ in 0..flips {
            let index = rng.below(data.len());
            data[index] = rng.below(256) as u8;
        }

        // Sonuç boş ya da geçerli olabilir; önemli olan panik atmaması ve sınırları aşmaması.
        let file = parse(&data);
        assert!(file.categories.len() <= JumpListLimits::MAX_CATEGORIES as usize);
        assert!(file.categories.iter().map(|c| c.items.len()).sum::<usize>() <= JumpListLimits::MAX_TOTAL_ITEMS as usize);
    }

    let noise: Vec<u8> = (0..4096).map(|_| rng.below(256) as u8).collect();
    assert!(parse(&noise).is_empty());
}

#[test]
fn header_encoding_is_utf16() {
    let file = parse(&CustomDestinationsBuilder::new().custom("İşlem ✓", &[link()]).build());

    assert_eq!(file.categories[0].name.as_deref(), Some("İşlem ✓"));
}
