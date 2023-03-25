use std::error::Error;

#[derive(Debug, Clone)]
pub enum TextFieldError {
    InvalidIndexPosition,
}

impl Error for TextFieldError {}

impl std::fmt::Display for TextFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            TextFieldError::InvalidIndexPosition => write!(f, "Invalid index position"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextField {
    text: String,
    index: usize,
}

impl Default for TextField {
    fn default() -> TextField {
        TextField::new()
    }
}

impl TextField {
    pub fn new() -> TextField {
        TextField {
            text: String::default(),
            index: 0,
        }
    }

    pub fn text(&self) -> String {
        self.text.clone()
    }

    pub fn set_text(&mut self, text: &String) {
        self.text = text.clone();
        self.index = self.text.len();
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set_index(&mut self, index: usize) -> Result<(), TextFieldError> {
        if (index >= 0) && (index <= self.text.len()) {
            self.index = index;
        } else {
            return Err(TextFieldError::InvalidIndexPosition);
        }

        Ok(())
    }

    pub fn clear(&mut self) {
        self.text = String::default();
        self.index = 0;
    }

    pub fn append_character(&mut self, character: char) {
        let mut text = self.text();
        text.push(character);
        self.set_text(&text);
        self.index = self.text.len();
    }

    pub fn append_string(&mut self, another_string: &String) {
        let mut text = self.text();
        text.push_str(another_string.as_str());
        self.set_text(&text);
        self.index = self.text.len();
    }

    pub fn insert_character(&mut self, character: char) {
        self.text.insert(self.index, character);
        self.index += 1;
    }

    pub fn insert_string(&mut self, another_string: &String) {
        self.text.insert_str(self.index, another_string.as_str());
    }

    pub fn left(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    pub fn right(&mut self) {
        if self.index < self.text.len() {
            self.index += 1;
        }
    }

    pub fn delete(&mut self) {
        if self.text.len() > 0 {
            self.text.remove(self.index);
        }
    }

    pub fn backspace(&mut self) {
        if self.index > 0 {
            self.left();
            self.delete();
        }
    }

    pub fn end(&mut self) {
        self.index = self.text.len();
    }

    pub fn home(&mut self) {
        self.index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_field() {
        let my_text = String::from("Some");
        let mut text = TextField::new();
        assert_eq!(text.index(), 0);
        assert!(text.is_empty());

        text.set_text(&my_text);
        assert_eq!(text.text(), my_text);
        assert_eq!(text.index(), 4);

        text.home();
        assert_eq!(text.index(), 0);

        text.end();
        assert_eq!(text.index(), 4);

        text.append_character(' ');
        let expected_text = r#"Some "#;
        assert_eq!(text.text(), expected_text);
        assert_eq!(text.index(), 5);

        text.append_string(&String::from("text"));
        let expected_text = r#"Some text"#;
        assert_eq!(text.text(), expected_text);
        assert_eq!(text.index(), 9);

        text.backspace();
        assert_eq!(text.index(), 8);
        assert_eq!(text.text(), "Some tex");

        text.left();
        text.left();
        text.left();
        assert_eq!(text.index(), 5);

        text.delete();
        assert_eq!(text.text(), "Some ex");
        assert_eq!(text.index(), 5);

        text.end();
        text.append_string(&String::from("tra"));
        assert_eq!(text.text(), "Some extra");
        assert_eq!(text.index(), 10);

        text.set_index(5).unwrap();
        assert_eq!(text.index(), 5);

        text.insert_character('?');
        assert_eq!(text.text(), "Some ?extra");

        text.delete();
        assert_eq!(text.text(), "Some extra");
        assert_eq!(text.index(), 5);

        text.left();
        assert_eq!(text.index(), 4);

        text.insert_string(&String::from("thing"));
        assert_eq!(text.text(), "Something extra");

        text.end();
        text.insert_string(&String::from(" special"));
        assert_eq!(text.text(), "Something extra special");

        text.home();
        for i in 0..10 {
            text.delete();
        }
        assert_eq!(text.text(), "extra special");

        text.home();
        text.insert_string(&String::from("You are "));
        assert_eq!(text.text(), "You are extra special");
    }
}
