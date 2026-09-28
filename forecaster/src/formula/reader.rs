//! `FormulaReader.java` — a char-cursor tokenizer with mark/reset, used by every formula-text
//! parser in this module (`KBIFormula`, `KBICode`, `Function` and its subclasses).

pub struct FormulaReader {
    text: Vec<char>,
    index: i64,
    mark_index: i64,
}

impl FormulaReader {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.chars().collect(),
            index: 0,
            mark_index: 0,
        }
    }

    pub fn has_next(&self) -> bool {
        self.index < self.text.len() as i64
    }

    pub fn has_previous(&self) -> bool {
        self.index >= 0
    }

    pub fn is_after_end(&self) -> bool {
        self.index >= self.text.len() as i64
    }

    pub fn is_before_start(&self) -> bool {
        self.index < 0
    }

    pub fn next(&mut self) -> char {
        let ch = self.text[self.index as usize];
        self.index += 1;
        ch
    }

    pub fn previous(&mut self) -> char {
        let ch = self.text[self.index as usize];
        self.index -= 1;
        ch
    }

    pub fn mark(&mut self) {
        self.mark_index = self.index;
    }

    pub fn reset(&mut self) {
        self.index = self.mark_index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_characters_in_order() {
        let mut reader = FormulaReader::new("ab");
        assert!(reader.has_next());
        assert_eq!(reader.next(), 'a');
        assert_eq!(reader.next(), 'b');
        assert!(!reader.has_next());
        assert!(reader.is_after_end());
    }

    #[test]
    fn mark_and_reset_rewind_to_the_marked_position() {
        let mut reader = FormulaReader::new("123abc");
        reader.mark();
        reader.next();
        reader.mark();
        reader.next();
        reader.next();
        reader.reset();
        assert_eq!(reader.next(), '2');
    }
}
