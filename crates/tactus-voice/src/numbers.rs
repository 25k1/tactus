const RUSSIAN: [(&str, u32); 52] = [
    ("ноль", 0),
    ("один", 1),
    ("одна", 1),
    ("одну", 1),
    ("одно", 1),
    ("два", 2),
    ("две", 2),
    ("три", 3),
    ("четыре", 4),
    ("пять", 5),
    ("шесть", 6),
    ("семь", 7),
    ("восемь", 8),
    ("девять", 9),
    ("десять", 10),
    ("одиннадцать", 11),
    ("двенадцать", 12),
    ("тринадцать", 13),
    ("четырнадцать", 14),
    ("пятнадцать", 15),
    ("шестнадцать", 16),
    ("семнадцать", 17),
    ("восемнадцать", 18),
    ("девятнадцать", 19),
    ("двадцать", 20),
    ("тридцать", 30),
    ("сорок", 40),
    ("пятьдесят", 50),
    ("шестьдесят", 60),
    ("семьдесят", 70),
    ("восемьдесят", 80),
    ("девяносто", 90),
    ("сто", 100),
    ("двести", 200),
    ("триста", 300),
    ("четыреста", 400),
    ("пятьсот", 500),
    ("шестьсот", 600),
    ("семьсот", 700),
    ("восемьсот", 800),
    ("девятьсот", 900),
    ("полчаса", 30),
    ("полминуты", 30),
    ("минуту", 1),
    ("минутку", 1),
    ("десяток", 10),
    ("пару", 2),
    ("пара", 2),
    ("час", 60),
    ("часик", 60),
    ("полтора", 90),
    ("полторы", 90),
];

const ENGLISH: [(&str, u32); 33] = [
    ("zero", 0),
    ("one", 1),
    ("a", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
    ("thirty", 30),
    ("forty", 40),
    ("fifty", 50),
    ("sixty", 60),
    ("seventy", 70),
    ("eighty", 80),
    ("ninety", 90),
    ("hundred", 100),
    ("half", 30),
    ("couple", 2),
    ("an", 1),
];

fn word_value(word: &str) -> Option<u32> {
    if let Ok(number) = word.parse::<u32>() {
        return (number <= 9_999).then_some(number);
    }
    RUSSIAN.iter().chain(&ENGLISH).find(|(name, _)| *name == word).map(|(_, value)| *value)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stage {
    Start,
    Hundreds,
    Tens,
    Done,
}

pub fn parse_number(words: &[&str]) -> Option<u32> {
    let words: Vec<&str> = words.iter().copied().filter(|word| *word != "and").collect();
    if let [word] = words.as_slice() {
        return word_value(word);
    }
    let mut total = 0u32;
    let mut stage = Stage::Start;
    for word in &words {
        let value = word_value(word)?;
        stage = match (stage, value) {
            (Stage::Start, 1..=9) if *word != "hundred" && words.get(1) == Some(&"hundred") => {
                total = value;
                Stage::Start
            }
            (Stage::Start, 100) if *word == "hundred" => {
                total = total.max(1) * 100;
                Stage::Hundreds
            }
            (Stage::Start, 100..=900) if value % 100 == 0 => {
                total = value;
                Stage::Hundreds
            }
            (Stage::Start | Stage::Hundreds, 20..=90) if value % 10 == 0 => {
                total += value;
                Stage::Tens
            }
            (Stage::Start | Stage::Hundreds | Stage::Tens, 1..=9) | (Stage::Start | Stage::Hundreds, 10..=19) => {
                total += value;
                Stage::Done
            }
            _ => return None,
        };
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> Option<u32> {
        parse_number(&text.split_whitespace().collect::<Vec<_>>())
    }

    #[test]
    fn reads_russian_numbers() {
        assert_eq!(number("тридцать пять"), Some(35));
        assert_eq!(number("сто двадцать"), Some(120));
        assert_eq!(number("одну"), Some(1));
        assert_eq!(number("полчаса"), Some(30));
        assert_eq!(number("пять тридцать"), None);
        assert_eq!(number("двести пять"), Some(205));
        assert_eq!(number("двадцать двадцать"), None);
        assert_eq!(number("полчаса"), Some(30));
    }

    #[test]
    fn reads_english_numbers_and_digits() {
        assert_eq!(number("forty two"), Some(42));
        assert_eq!(number("one hundred and five"), Some(105));
        assert_eq!(number("a hundred"), Some(100));
        assert_eq!(number("75"), Some(75));
        assert_eq!(number("ten five"), None);
        assert_eq!(number("twenty five"), Some(25));
        assert_eq!(number("two hundred"), Some(200));
        assert_eq!(number("hundred twenty"), Some(120));
        assert_eq!(number("loud"), None);
    }
}
