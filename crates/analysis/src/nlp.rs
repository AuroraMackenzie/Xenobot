use crate::error::{AnalysisError, AnalysisResult};
use jieba_rs::Jieba;
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;

/// Chinese text segmenter using jieba-rs.
pub struct ChineseSegmenter {
    jieba: Arc<Jieba>,
}

impl ChineseSegmenter {
    /// Create a new ChineseSegmenter with default dictionary.
    pub fn new() -> Self {
        Self {
            jieba: Arc::new(Jieba::new()),
        }
    }

    /// Create a new ChineseSegmenter with custom dictionary.
    pub fn with_dict(_dict_path: &str) -> AnalysisResult<Self> {
        let jieba = Jieba::new();
        // TODO: Load custom dictionary
        // jieba.load_dict(dict_path).map_err(|e| AnalysisError::Nlp(e.to_string()))?;
        Ok(Self {
            jieba: Arc::new(jieba),
        })
    }

    /// Segment Chinese text into words.
    pub fn segment(&self, text: &str) -> AnalysisResult<Vec<String>> {
        let words = self.jieba.cut(text, false);
        Ok(words.into_iter().map(String::from).collect())
    }

    /// Segment Chinese text with HMM (Hidden Markov Model) for unknown words.
    pub fn segment_hmm(&self, text: &str) -> AnalysisResult<Vec<String>> {
        let words = self.jieba.cut(text, true);
        Ok(words.into_iter().map(String::from).collect())
    }

    /// Extract keywords from Chinese text using TF-IDF.
    pub fn extract_keywords(&self, text: &str, top_k: usize) -> AnalysisResult<Vec<(String, f64)>> {
        let words = self.segment(text)?;

        // Simple TF calculation (in production, use proper TF-IDF)
        let mut word_counts = std::collections::HashMap::new();
        for word in &words {
            if word.len() > 1 {
                // Filter single character words
                *word_counts.entry(word.clone()).or_insert(0) += 1;
            }
        }

        let total_words = words.len() as f64;
        let mut keywords: Vec<_> = word_counts
            .into_iter()
            .map(|(word, count)| (word, count as f64 / total_words))
            .collect();

        keywords.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        keywords.truncate(top_k);

        Ok(keywords)
    }
}

/// Stopwords filter for Chinese text.
pub struct ChineseStopwords {
    stopwords: HashSet<String>,
}

impl ChineseStopwords {
    /// Create a new ChineseStopwords with default stopword list.
    pub fn new() -> Self {
        let mut stopwords = HashSet::new();
        let default_stopwords = [
            "的",
            "了",
            "在",
            "是",
            "我",
            "有",
            "和",
            "就",
            "不",
            "人",
            "都",
            "一",
            "一个",
            "上",
            "也",
            "很",
            "到",
            "说",
            "要",
            "去",
            "你",
            "会",
            "着",
            "没有",
            "看",
            "好",
            "自己",
            "这",
            "那",
            "她",
            "他",
            "我们",
            "你们",
            "他们",
            "这个",
            "那个",
            "什么",
            "怎么",
            "为什么",
            "因为",
            "所以",
            "但是",
            "虽然",
            "如果",
            "然后",
            "而且",
            "或者",
            "还是",
            "可以",
            "可能",
            "应该",
            "一些",
            "一点",
            "一些",
            "一切",
            "一样",
            "一般",
            "一直",
            "一起",
            "一下",
            "一定",
            "一种",
            "不会",
            "不能",
            "不要",
            "这样",
            "那样",
            "这些",
            "那些",
            "这里",
            "那里",
            "这么",
            "那么",
            "这样",
            "那样",
            "这边",
            "那边",
            "这样",
            "那样",
            "吧",
            "呢",
            "吗",
            "啊",
            "呀",
            "哦",
            "嗯",
            "呃",
            "呵",
            "哈",
            "唉",
            "喂",
            "哇",
            "啦",
            "嘛",
            "呗",
            "喽",
            "哟",
            "哉",
            "之",
            "乎",
            "者",
            "也",
            "焉",
            "哉",
            "兮",
            "耶",
            "欤",
            "而已",
            "罢了",
            "的话",
            "来说",
            "来讲",
            "来看",
            "来说",
            "于",
            "与",
            "以",
            "而",
            "且",
            "或",
            "乃",
            "及",
            "暨",
            "跟",
            "同",
            "和",
            "与",
            "并",
            "且",
            "而",
            "或",
            "或者",
            "从",
            "自",
            "自从",
            "于",
            "打",
            "到",
            "至",
            "往",
            "在",
            "当",
            "朝",
            "向",
            "顺着",
            "沿着",
            "随着",
            "按",
            "照",
            "按照",
            "依",
            "依照",
            "本着",
            "通过",
            "根据",
            "以",
            "凭",
            "为",
            "为了",
            "为着",
            "由于",
            "因为",
            "因此",
            "所以",
            "但是",
            "但",
            "然而",
            "却",
            "反而",
            "不过",
            "只是",
            "只是",
            "可是",
            "可",
            "却",
            "而",
            "而且",
            "并且",
            "何况",
            "甚至",
            "尤其",
            "特别",
            "更",
            "更加",
            "越",
            "越发",
            "稍微",
            "稍稍",
            "略微",
            "比较",
            "较",
            "较之",
            "相对",
            "相对而言",
        ];

        for word in default_stopwords.iter() {
            stopwords.insert(word.to_string());
        }

        Self { stopwords }
    }

    /// Create a new ChineseStopwords with custom stopword list.
    pub fn with_custom_list(stopwords: Vec<String>) -> Self {
        Self {
            stopwords: stopwords.into_iter().collect(),
        }
    }

    /// Check if a word is a stopword.
    pub fn is_stopword(&self, word: &str) -> bool {
        self.stopwords.contains(word)
    }

    /// Filter stopwords from a list of words.
    pub fn filter_stopwords(&self, words: Vec<String>) -> Vec<String> {
        words
            .into_iter()
            .filter(|word| !self.is_stopword(word))
            .collect()
    }
}

/// Named entity recognizer for Chinese text.
pub struct ChineseNER {
    regexes: Vec<(Regex, &'static str)>,
}

impl ChineseNER {
    /// Create a new ChineseNER with default patterns.
    pub fn new() -> AnalysisResult<Self> {
        let regexes = vec![
            // Person names (Chinese names typically 2-4 characters)
            (
                Regex::new(r"[\u4e00-\u9fff]{2,4}")
                    .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "PERSON",
            ),
            // Phone numbers
            (
                Regex::new(r"1[3-9]\d{9}").map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "PHONE",
            ),
            // Email addresses
            (
                Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}")
                    .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "EMAIL",
            ),
            // URLs
            (
                Regex::new(r"https?://[^\s]+").map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "URL",
            ),
            // Time expressions
            (
                Regex::new(r"\d{1,2}[:：]\d{1,2}")
                    .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "TIME",
            ),
            // Dates
            (
                Regex::new(r"\d{4}[-年]\d{1,2}[-月]\d{1,2}[日号]?")
                    .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "DATE",
            ),
            // Numbers
            (
                Regex::new(r"\d+(\.\d+)?").map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "NUMBER",
            ),
            // Currency
            (
                Regex::new(r"[¥￥$€£]\s?\d+(\.\d+)?")
                    .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
                "CURRENCY",
            ),
        ];

        Ok(Self { regexes })
    }

    /// Recognize named entities in text.
    pub fn recognize(&self, text: &str) -> AnalysisResult<Vec<(String, String, usize, usize)>> {
        let mut entities = Vec::new();

        for (regex, entity_type) in &self.regexes {
            for mat in regex.find_iter(text) {
                entities.push((
                    entity_type.to_string(),
                    mat.as_str().to_string(),
                    mat.start(),
                    mat.end(),
                ));
            }
        }

        // Remove overlapping entities (keep longer ones)
        entities.sort_by(|a, b| a.2.cmp(&b.2).then(b.3.cmp(&a.3)));
        let mut filtered = Vec::new();
        let mut last_end = 0;

        for entity in entities {
            if entity.2 >= last_end {
                let end = entity.3;
                filtered.push(entity);
                last_end = end;
            }
        }

        Ok(filtered)
    }
}

/// Text normalizer for Chinese text.
pub struct ChineseTextNormalizer {
    punctuation_regex: Regex,
    whitespace_regex: Regex,
}

impl ChineseTextNormalizer {
    /// Create a new ChineseTextNormalizer.
    pub fn new() -> AnalysisResult<Self> {
        Ok(Self {
            punctuation_regex: Regex::new(r#"[，。！？；：、（）《》【】「」『』"'"—…~·]"#)
                .map_err(|e| AnalysisError::Nlp(e.to_string()))?,
            whitespace_regex: Regex::new(r"\s+").map_err(|e| AnalysisError::Nlp(e.to_string()))?,
        })
    }

    /// Normalize Chinese text by removing excessive punctuation and whitespace.
    pub fn normalize(&self, text: &str) -> AnalysisResult<String> {
        let text = self.punctuation_regex.replace_all(text, " ");
        let text = self.whitespace_regex.replace_all(&text, " ");
        Ok(text.trim().to_string())
    }

    /// Convert full-width characters to half-width.
    pub fn to_halfwidth(&self, text: &str) -> String {
        let mut normalized = String::with_capacity(text.len());

        for c in text.chars() {
            match c {
                '，' => normalized.push(','),
                '。' => normalized.push('.'),
                '！' => normalized.push('!'),
                '？' => normalized.push('?'),
                '；' => normalized.push(';'),
                '：' => normalized.push(':'),
                '（' => normalized.push('('),
                '）' => normalized.push(')'),
                '《' => normalized.push('<'),
                '》' => normalized.push('>'),
                '【' => normalized.push('['),
                '】' => normalized.push(']'),
                '「' => normalized.push('{'),
                '」' => normalized.push('}'),
                '『' => normalized.push('['),
                '』' => normalized.push(']'),
                '“' => normalized.push('"'),
                '”' => normalized.push('"'),
                '‘' => normalized.push('\''),
                '’' => normalized.push('\''),
                '—' => normalized.push('-'),
                '…' => normalized.push_str("..."),
                '～' => normalized.push('~'),
                '·' => normalized.push('`'),
                '０' => normalized.push('0'),
                '１' => normalized.push('1'),
                '２' => normalized.push('2'),
                '３' => normalized.push('3'),
                '４' => normalized.push('4'),
                '５' => normalized.push('5'),
                '６' => normalized.push('6'),
                '７' => normalized.push('7'),
                '８' => normalized.push('8'),
                '９' => normalized.push('9'),
                'Ａ' => normalized.push('A'),
                'Ｂ' => normalized.push('B'),
                'Ｃ' => normalized.push('C'),
                'Ｄ' => normalized.push('D'),
                'Ｅ' => normalized.push('E'),
                'Ｆ' => normalized.push('F'),
                'Ｇ' => normalized.push('G'),
                'Ｈ' => normalized.push('H'),
                'Ｉ' => normalized.push('I'),
                'Ｊ' => normalized.push('J'),
                'Ｋ' => normalized.push('K'),
                'Ｌ' => normalized.push('L'),
                'Ｍ' => normalized.push('M'),
                'Ｎ' => normalized.push('N'),
                'Ｏ' => normalized.push('O'),
                'Ｐ' => normalized.push('P'),
                'Ｑ' => normalized.push('Q'),
                'Ｒ' => normalized.push('R'),
                'Ｓ' => normalized.push('S'),
                'Ｔ' => normalized.push('T'),
                'Ｕ' => normalized.push('U'),
                'Ｖ' => normalized.push('V'),
                'Ｗ' => normalized.push('W'),
                'Ｘ' => normalized.push('X'),
                'Ｙ' => normalized.push('Y'),
                'Ｚ' => normalized.push('Z'),
                'ａ' => normalized.push('a'),
                'ｂ' => normalized.push('b'),
                'ｃ' => normalized.push('c'),
                'ｄ' => normalized.push('d'),
                'ｅ' => normalized.push('e'),
                'ｆ' => normalized.push('f'),
                'ｇ' => normalized.push('g'),
                'ｈ' => normalized.push('h'),
                'ｉ' => normalized.push('i'),
                'ｊ' => normalized.push('j'),
                'ｋ' => normalized.push('k'),
                'ｌ' => normalized.push('l'),
                'ｍ' => normalized.push('m'),
                'ｎ' => normalized.push('n'),
                'ｏ' => normalized.push('o'),
                'ｐ' => normalized.push('p'),
                'ｑ' => normalized.push('q'),
                'ｒ' => normalized.push('r'),
                'ｓ' => normalized.push('s'),
                'ｔ' => normalized.push('t'),
                'ｕ' => normalized.push('u'),
                'ｖ' => normalized.push('v'),
                'ｗ' => normalized.push('w'),
                'ｘ' => normalized.push('x'),
                'ｙ' => normalized.push('y'),
                'ｚ' => normalized.push('z'),
                _ => normalized.push(c),
            }
        }

        normalized
    }
}
