pub fn normalize(text: &str) -> String {
    let cleaned: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'ё' => 'е',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, left) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let substitution = previous + usize::from(left != *right);
            previous = row[j + 1];
            row[j + 1] = substitution.min(row[j] + 1).min(previous + 1);
        }
    }
    row[b.len()]
}

pub fn close(word: &str, heard: &str) -> bool {
    let length = word.chars().count();
    let allowed = match length {
        0..=4 => 0,
        5..=7 => 1,
        _ => 2,
    };
    allowed > 0 && distance(word, heard) <= allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_punctuation_and_yo() {
        assert_eq!(normalize("  Включи, «Ёлку»!  "), "включи елку");
        assert_eq!(normalize("Play AC/DC"), "play ac dc");
    }

    #[test]
    fn tolerates_small_errors_in_longer_words() {
        assert_eq!(distance("следующий", "следущий"), 1);
        assert!(close("следующий", "следущий"));
        assert!(!close("тише", "тиши"));
        assert!(!close("громче", "тише"));
    }
}
