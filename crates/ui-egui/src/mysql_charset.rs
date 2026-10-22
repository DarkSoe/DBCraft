//! MySQL/MariaDB character sets and common collations (Navicat-style lists).

/// All charsets typically shown in Navicat's "Zeichensatz" dropdown.
pub fn mysql_charsets() -> &'static [&'static str] {
    &[
        "armscii8",
        "ascii",
        "big5",
        "binary",
        "cp1250",
        "cp1251",
        "cp1256",
        "cp1257",
        "cp850",
        "cp852",
        "cp866",
        "cp932",
        "dec8",
        "eucjpms",
        "euckr",
        "gb18030",
        "gb2312",
        "gbk",
        "geostd8",
        "greek",
        "hebrew",
        "hp8",
        "keybcs2",
        "koi8r",
        "koi8u",
        "latin1",
        "latin2",
        "latin5",
        "latin7",
        "macce",
        "macroman",
        "sjis",
        "swe7",
        "tis620",
        "ucs2",
        "ujis",
        "utf8mb3",
        "utf8mb4",
        "utf16",
        "utf16le",
        "utf32",
    ]
}

/// Common collations for a charset (empty = free text / server default).
pub fn collations_for(charset: &str) -> &'static [&'static str] {
    match charset {
        "utf8mb4" => &[
            "utf8mb4_general_ci",
            "utf8mb4_unicode_ci",
            "utf8mb4_unicode_520_ci",
            "utf8mb4_bin",
            "utf8mb4_0900_ai_ci",
            "utf8mb4_german2_ci",
            "utf8mb4_croatian_ci",
        ],
        "utf8mb3" | "utf8" => &[
            "utf8_general_ci",
            "utf8_unicode_ci",
            "utf8_bin",
            "utf8_german2_ci",
        ],
        "latin1" => &[
            "latin1_swedish_ci",
            "latin1_general_ci",
            "latin1_german1_ci",
            "latin1_german2_ci",
            "latin1_bin",
        ],
        "latin2" => &["latin2_general_ci", "latin2_bin"],
        "ascii" => &["ascii_general_ci", "ascii_bin"],
        "binary" => &["binary"],
        "ucs2" => &["ucs2_general_ci", "ucs2_unicode_ci", "ucs2_bin"],
        "utf16" => &["utf16_general_ci", "utf16_unicode_ci", "utf16_bin"],
        "utf32" => &["utf32_general_ci", "utf32_unicode_ci", "utf32_bin"],
        "cp1250" => &["cp1250_general_ci", "cp1250_bin"],
        "cp1251" => &["cp1251_general_ci", "cp1251_bin"],
        "gbk" => &["gbk_chinese_ci", "gbk_bin"],
        "sjis" => &["sjis_japanese_ci", "sjis_bin"],
        _ => &[],
    }
}

pub fn default_collation(charset: &str) -> &'static str {
    collations_for(charset).first().copied().unwrap_or("")
}
